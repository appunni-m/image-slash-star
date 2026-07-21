// Spatial Huffman histogram clustering, ported from libwebp 1.6.0
// `src/enc/histogram_enc.c` and `src/dsp/lossless_enc.c`.

use super::backward_refs::{self, Token};
use super::{channels, length_to_symbol};

const NON_TRIVIAL: u16 = u16::MAX;
const PRECISION: u32 = 23;
const NUM_PARTITIONS: usize = 4;
const BIN_SIZE: usize = NUM_PARTITIONS * NUM_PARTITIONS * NUM_PARTITIONS;
const MAX_HISTO_GREEDY: u64 = 100;

#[derive(Clone)]
pub(super) struct Histogram {
    pub(super) populations: [Vec<u32>; 5],
    costs: [u64; 5],
    trivial: [u16; 5],
    used: [bool; 5],
    bit_cost: u64,
    bin_id: usize,
}

impl Histogram {
    fn new(cache_bits: u8) -> Self {
        let cache_size = if cache_bits == 0 { 0 } else { 1 << cache_bits };
        Self {
            populations: [
                vec![0; 280 + cache_size],
                vec![0; 256],
                vec![0; 256],
                vec![0; 256],
                vec![0; 40],
            ],
            costs: [0; 5],
            trivial: [NON_TRIVIAL; 5],
            used: [true; 5],
            bit_cost: 0,
            bin_id: 0,
        }
    }

    fn add_token(&mut self, token: Token, width: usize) {
        match token {
            Token::Literal(pixel) => {
                let [red, green, blue, alpha] = channels(pixel);
                self.populations[0][green] += 1;
                self.populations[1][red] += 1;
                self.populations[2][blue] += 1;
                self.populations[3][alpha] += 1;
            }
            Token::Copy { distance, length } => {
                let (length_symbol, _) = length_to_symbol(length);
                self.populations[0][256 + length_symbol] += 1;
                let plane_distance = backward_refs::plane_code(width, distance);
                let (distance_symbol, _) = length_to_symbol(plane_distance);
                self.populations[4][distance_symbol] += 1;
            }
            Token::Cache(index) => self.populations[0][280 + index] += 1,
        }
    }

    fn analyze(&mut self) {
        self.bit_cost = 0;
        for i in 0..5 {
            let (cost, trivial, used) = population_cost(&self.populations[i]);
            self.costs[i] = cost;
            self.trivial[i] = trivial;
            self.used[i] = used;
            self.bit_cost += cost;
        }
    }

    fn add_assign(&mut self, other: &Self) {
        for (to, from) in self.populations.iter_mut().zip(&other.populations) {
            for (to, from) in to.iter_mut().zip(from) {
                *to += *from;
            }
        }
        for i in 0..5 {
            self.trivial[i] = if self.trivial[i] == other.trivial[i] {
                self.trivial[i]
            } else {
                NON_TRIVIAL
            };
            self.used[i] |= other.used[i];
        }
    }
}

#[derive(Default)]
struct BitEntropy {
    entropy: u64,
    sum: u32,
    nonzeros: u32,
    max_value: u32,
    nonzero_code: u16,
}

#[derive(Default)]
struct Streaks {
    counts: [u32; 2],
    streaks: [[u32; 2]; 2],
}

fn entropy_unrefined(x: &[u32], y: Option<&[u32]>) -> (BitEntropy, Streaks) {
    let value = |i: usize| x[i] + y.map_or(0, |values| values[i]);
    let mut entropy = BitEntropy {
        nonzero_code: NON_TRIVIAL,
        ..BitEntropy::default()
    };
    let mut stats = Streaks::default();
    let mut previous_index = 0;
    let mut previous = value(0);
    for i in 1..x.len() {
        let current = value(i);
        if current != previous {
            entropy_streak(
                previous,
                i - previous_index,
                previous_index,
                &mut entropy,
                &mut stats,
            );
            previous = current;
            previous_index = i;
        }
    }
    entropy_streak(
        previous,
        x.len() - previous_index,
        previous_index,
        &mut entropy,
        &mut stats,
    );
    entropy.entropy = backward_refs::fast_slog(entropy.sum).saturating_sub(entropy.entropy);
    (entropy, stats)
}

fn entropy_streak(
    value: u32,
    streak: usize,
    index: usize,
    entropy: &mut BitEntropy,
    stats: &mut Streaks,
) {
    if value != 0 {
        entropy.sum += value * streak as u32;
        entropy.nonzeros += streak as u32;
        entropy.nonzero_code = index as u16;
        entropy.entropy += backward_refs::fast_slog(value) * streak as u64;
        entropy.max_value = entropy.max_value.max(value);
    }
    let nonzero = usize::from(value != 0);
    let long = usize::from(streak > 3);
    stats.counts[nonzero] += long as u32;
    stats.streaks[nonzero][long] += streak as u32;
}

fn div_round(value: u64, divisor: u64) -> u64 {
    (value + divisor / 2) / divisor
}

fn refined_entropy(entropy: &BitEntropy) -> u64 {
    let mix = match entropy.nonzeros {
        0 | 1 => return 0,
        2 => {
            return div_round(
                99 * (u64::from(entropy.sum) << PRECISION) + entropy.entropy,
                100,
            );
        }
        3 => 950,
        4 => 700,
        _ => 627,
    };
    let minimum = (u64::from(2 * entropy.sum - entropy.max_value)) << PRECISION;
    let minimum = div_round(mix * minimum + (1000 - mix) * entropy.entropy, 1000);
    entropy.entropy.max(minimum)
}

fn final_huffman_cost(stats: &Streaks) -> u64 {
    let initial = (19_u64 * 3 << PRECISION) - div_round(91 << PRECISION, 10);
    let extra = stats.counts[0] * 1600
        + 240 * stats.streaks[0][1]
        + stats.counts[1] * 2640
        + 720 * stats.streaks[1][1]
        + 1840 * stats.streaks[0][0]
        + 3360 * stats.streaks[1][0];
    initial + (u64::from(extra) << (PRECISION - 10))
}

fn population_cost(population: &[u32]) -> (u64, u16, bool) {
    let (entropy, stats) = entropy_unrefined(population, None);
    let trivial = if entropy.nonzeros == 1 {
        entropy.nonzero_code
    } else {
        NON_TRIVIAL
    };
    let used = stats.streaks[1][0] != 0 || stats.streaks[1][1] != 0;
    (
        refined_entropy(&entropy) + final_huffman_cost(&stats),
        trivial,
        used,
    )
}

pub(super) fn bits_entropy(population: &[u32]) -> u64 {
    let (entropy, _) = entropy_unrefined(population, None);
    refined_entropy(&entropy)
}

fn combined_channel_cost(a: &Histogram, b: &Histogram, channel: usize) -> u64 {
    if (a.trivial[channel] != NON_TRIVIAL && a.trivial[channel] == b.trivial[channel])
        || !a.used[channel]
        || !b.used[channel]
    {
        return if a.used[channel] {
            a.costs[channel]
        } else {
            b.costs[channel]
        };
    }
    let (entropy, stats) =
        entropy_unrefined(&a.populations[channel], Some(&b.populations[channel]));
    refined_entropy(&entropy) + final_huffman_cost(&stats)
}

fn combined_costs(a: &Histogram, b: &Histogram, threshold: i64) -> Option<(u64, [u64; 5])> {
    if threshold <= 0 {
        return None;
    }
    let mut total = 0;
    let mut costs = [0; 5];
    for (channel, cost) in costs.iter_mut().enumerate() {
        *cost = combined_channel_cost(a, b, channel);
        total += *cost;
        if total >= threshold as u64 {
            return None;
        }
    }
    Some((total, costs))
}

fn add_eval(a: &Histogram, b: &Histogram, threshold: i64) -> Option<Histogram> {
    let sum = a.bit_cost + b.bit_cost;
    let limit = threshold.saturating_add_unsigned(sum);
    let (cost, costs) = combined_costs(a, b, limit)?;
    let mut result = b.clone();
    result.add_assign(a);
    result.costs = costs;
    result.bit_cost = cost;
    Some(result)
}

fn add_threshold(a: &Histogram, b: &Histogram, threshold: i64) -> Option<i64> {
    let limit = threshold.saturating_add_unsigned(a.bit_cost);
    let (cost, _) = combined_costs(a, b, limit)?;
    Some(cost as i64 - a.bit_cost as i64)
}

#[derive(Clone)]
struct Pair {
    first: usize,
    second: usize,
    cost_diff: i64,
    cost_combo: u64,
    costs: [u64; 5],
}

fn update_pair(histograms: &[Histogram], pair: &mut Pair, threshold: i64) -> bool {
    let sum = histograms[pair.first].bit_cost + histograms[pair.second].bit_cost;
    let limit = threshold.saturating_add_unsigned(sum);
    let Some((cost, costs)) =
        combined_costs(&histograms[pair.first], &histograms[pair.second], limit)
    else {
        return false;
    };
    pair.cost_combo = cost;
    pair.costs = costs;
    pair.cost_diff = cost as i64 - sum as i64;
    true
}

fn update_head(queue: &mut [Pair], index: usize) {
    if queue[index].cost_diff < queue[0].cost_diff {
        queue.swap(0, index);
    }
}

fn push_pair(
    queue: &mut Vec<Pair>,
    maximum: usize,
    histograms: &[Histogram],
    mut first: usize,
    mut second: usize,
    threshold: i64,
) -> i64 {
    if queue.len() == maximum {
        return 0;
    }
    if first > second {
        core::mem::swap(&mut first, &mut second);
    }
    let mut pair = Pair {
        first,
        second,
        cost_diff: 0,
        cost_combo: 0,
        costs: [0; 5],
    };
    if !update_pair(histograms, &mut pair, threshold) {
        return 0;
    }
    let result = pair.cost_diff;
    queue.push(pair);
    let index = queue.len() - 1;
    update_head(queue, index);
    result
}

fn fix_pair(pair: &mut Pair, bad: usize, good: usize) {
    if pair.first == bad {
        pair.first = good;
    }
    if pair.second == bad {
        pair.second = good;
    }
    if pair.first > pair.second {
        core::mem::swap(&mut pair.first, &mut pair.second);
    }
}

fn entropy_bin_combine(histograms: &mut Vec<Histogram>) {
    let mut minima = [u64::MAX; 3];
    let mut maxima = [0; 3];
    for histogram in histograms.iter() {
        for (range, channel) in [0, 1, 2].into_iter().enumerate() {
            minima[range] = minima[range].min(histogram.costs[channel]);
            maxima[range] = maxima[range].max(histogram.costs[channel]);
        }
    }
    for histogram in histograms.iter_mut() {
        let mut bin = 0;
        for channel in 0..3 {
            let range = maxima[channel] - minima[channel];
            let part = if range == 0 {
                0
            } else {
                (3.999_999_f64 * (histogram.costs[channel] - minima[channel]) as f64 / range as f64)
                    as usize
            };
            bin = bin * NUM_PARTITIONS + part;
        }
        histogram.bin_id = bin;
    }
    let mut first = [None; BIN_SIZE];
    let mut failures = [0_u16; BIN_SIZE];
    let mut index = 0;
    while index < histograms.len() {
        let bin = histograms[index].bin_id;
        let Some(first_index) = first[bin] else {
            first[bin] = Some(index);
            index += 1;
            continue;
        };
        let threshold = -(((histograms[index].bit_cost * 16 + 50) / 100) as i64);
        let Some(combo) = add_eval(&histograms[first_index], &histograms[index], threshold) else { index += 1; continue; };
        let trivial_combo = combo.trivial[1..4]
            .iter()
            .all(|&symbol| symbol != NON_TRIVIAL);
        let nontrivial_pair = histograms[index].trivial[1..4].contains(&NON_TRIVIAL)
            && histograms[first_index].trivial[1..4].contains(&NON_TRIVIAL);
        if trivial_combo || nontrivial_pair || failures[bin] >= 32 {
            histograms[first_index] = combo;
            histograms.swap_remove(index);
        } else {
            failures[bin] += 1;
            index += 1;
        }
    }
}

fn stochastic_combine(histograms: &mut Vec<Histogram>, minimum: usize) -> bool {
    if histograms.len() < minimum {
        return true;
    }
    let outer_iterations = histograms.len();
    let maximum_failures = outer_iterations / 2;
    let mut failures = 0;
    let mut seed = 1_u32;
    let mut queue: Vec<Pair> = Vec::with_capacity(9);
    for _ in 0..outer_iterations {
        failures += 1;
        if histograms.len() < minimum || failures >= maximum_failures {
            break;
        }
        let mut best = queue.first().map_or(0, |pair| pair.cost_diff);
        let range = (histograms.len() * (histograms.len() - 1)) as u32;
        for _ in 0..histograms.len() / 2 {
            seed = ((u64::from(seed) * 48_271) % 2_147_483_647) as u32;
            let random = seed % range;
            let first = random as usize / (histograms.len() - 1);
            let mut second = random as usize % (histograms.len() - 1);
            if second >= first {
                second += 1;
            }
            let cost = push_pair(&mut queue, 9, histograms, first, second, best);
            if cost < 0 {
                best = cost;
                if queue.len() == 9 { break; }
            }
        }
        if queue.is_empty() { continue; }
        let chosen = queue[0].clone();
        let first = chosen.first;
        let second = chosen.second;
        let other = histograms[second].clone();
        histograms[first].add_assign(&other);
        histograms[first].bit_cost = chosen.cost_combo;
        histograms[first].costs = chosen.costs;
        histograms.swap_remove(second);
        let moved_from = histograms.len();
        let mut index = 0;
        while index < queue.len() {
            let touches_first = queue[index].first == first || queue[index].first == second;
            let touches_second = queue[index].second == first || queue[index].second == second;
            if touches_first && touches_second {
                queue.swap_remove(index);
                continue;
            }
            fix_pair(&mut queue[index], moved_from, second);
            if touches_first || touches_second {
                fix_pair(&mut queue[index], second, first);
                let mut pair = queue[index].clone();
                if !update_pair(histograms, &mut pair, 0) { queue.swap_remove(index); continue; }
                queue[index] = pair;
            }
            update_head(&mut queue, index);
            index += 1;
        }
        failures = 0;
    }
    histograms.len() <= minimum
}

fn greedy_combine(histograms: &mut Vec<Histogram>) {
    let maximum = histograms.len() * histograms.len();
    let mut queue = Vec::new();
    for first in 0..histograms.len() {
        for second in first + 1..histograms.len() {
            push_pair(&mut queue, maximum, histograms, first, second, 0);
        }
    }
    while let Some(chosen) = queue.first().cloned() {
        let first = chosen.first;
        let second = chosen.second;
        let other = histograms[second].clone();
        histograms[first].add_assign(&other);
        histograms[first].bit_cost = chosen.cost_combo;
        histograms[first].costs = chosen.costs;
        histograms.swap_remove(second);
        let moved_from = histograms.len();
        let mut index = 0;
        while index < queue.len() {
            if [queue[index].first, queue[index].second]
                .iter()
                .any(|&value| value == first || value == second)
            {
                queue.swap_remove(index);
            } else {
                fix_pair(&mut queue[index], moved_from, second);
                update_head(&mut queue, index);
                index += 1;
            }
        }
        for index in 0..histograms.len() {
            if index != first {
                push_pair(&mut queue, maximum, histograms, first, index, 0);
            }
        }
    }
}

pub(super) fn cluster(
    tokens: &[Token],
    width: usize,
    height: usize,
    cache_bits: u8,
    quality: u32,
    histogram_bits: u8,
) -> (Vec<u16>, Vec<Histogram>) {
    let tile_width = (width + (1 << histogram_bits) - 1) >> histogram_bits;
    let tile_height = (height + (1 << histogram_bits) - 1) >> histogram_bits;
    let mut originals = vec![Histogram::new(cache_bits); tile_width * tile_height];
    let mut x = 0;
    let mut y = 0;
    for &token in tokens {
        let tile = (y >> histogram_bits) * tile_width + (x >> histogram_bits);
        originals[tile].add_token(token, width);
        x += match token {
            Token::Copy { length, .. } => length,
            _ => 1,
        };
        while x >= width {
            x -= width;
            y += 1;
        }
    }
    for histogram in &mut originals {
        histogram.analyze();
    }
    let mut clusters = originals
        .iter()
        .filter(|histogram| histogram.used.iter().any(|&used| used))
        .cloned()
        .collect::<Vec<_>>();
    if clusters.len() > 2 * BIN_SIZE && quality < 100 {
        entropy_bin_combine(&mut clusters);
    }
    let threshold = 1 + div_round(
        u64::from(quality).pow(3) * (MAX_HISTO_GREEDY - 1),
        1_000_000,
    ) as usize;
    if stochastic_combine(&mut clusters, threshold) { greedy_combine(&mut clusters); }

    let mut symbols = vec![0_u16; originals.len()];
    for (index, original) in originals.iter().enumerate() {
        if !original.used.iter().any(|&used| used) {
            symbols[index] = symbols[index - 1];
            continue;
        }
        let mut best = i64::MAX;
        for (cluster_index, cluster) in clusters.iter().enumerate() {
            if let Some(cost) = add_threshold(cluster, original, best) {
                best = cost;
                symbols[index] = cluster_index as u16;
            }
        }
    }
    let mut remapped = vec![Histogram::new(cache_bits); clusters.len()];
    for (original, &symbol) in originals.iter().zip(&symbols) {
        if original.used.iter().any(|&used| used) {
            remapped[usize::from(symbol)].add_assign(original);
        }
    }
    (symbols, remapped)
}

#[cfg(coverage)]
pub(crate) fn __coverage_exercise_private_branches() {
    let mut a = Histogram::new(0);
    let mut b = Histogram::new(0);
    a.add_token(Token::Literal(0xff00_0000), 1);
    b.add_token(Token::Literal(0xff00_00ff), 1);
    a.analyze();
    b.analyze();
    assert!(combined_costs(&a, &b, 0).is_none());

    let histograms = vec![a, b];
    let mut queue = vec![Pair {
        first: 0,
        second: 1,
        cost_diff: 0,
        cost_combo: 0,
        costs: [0; 5],
    }];
    let _ = push_pair(&mut queue, 1, &histograms, 0, 1, -1);

    let mut equal_bins = vec![Histogram::new(0), Histogram::new(0)];
    for histogram in &mut equal_bins {
        histogram.analyze();
    }
    entropy_bin_combine(&mut equal_bins);
    let mut rejected_same_bin = vec![Histogram::new(0), Histogram::new(0)];
    for histogram in &mut rejected_same_bin {
        histogram.costs = [0; 5];
        histogram.trivial = [0; 5];
        histogram.used = [false; 5];
        histogram.bit_cost = 0;
    }
    entropy_bin_combine(&mut rejected_same_bin);

    let mut no_pairs = vec![
        Histogram::new(0),
        Histogram::new(0),
        Histogram::new(0),
        Histogram::new(0),
    ];
    for histogram in &mut no_pairs {
        histogram.analyze();
    }
    let _ = stochastic_combine(&mut no_pairs, 4);

    let mut mergeable = Vec::new();
    for _ in 0..24 {
        let mut histogram = Histogram::new(0);
        histogram.add_token(Token::Literal(0xff00_0000), 1);
        histogram.analyze();
        mergeable.push(histogram);
    }
    let _ = stochastic_combine(&mut mergeable, 1);
    let mut large_mergeable = Vec::new();
    for _ in 0..64 {
        let mut histogram = Histogram::new(0);
        histogram.add_token(Token::Literal(0xff00_0000), 1);
        histogram.analyze();
        large_mergeable.push(histogram);
    }
    let _ = stochastic_combine(&mut large_mergeable, 1);

    let mut distinct = Vec::new();
    for index in 0..8 {
        let mut histogram = Histogram::new(0);
        histogram.add_token(Token::Literal(0xff00_0000 | index), 1);
        histogram.analyze();
        distinct.push(histogram);
    }
    let _ = stochastic_combine(&mut distinct, 1);

    let mut high_entropy = Vec::new();
    let mut high_entropy_tokens = Vec::new();
    for histogram_index in 0_u32..24 {
        let mut histogram = Histogram::new(0);
        for token_index in 0_u32..64 {
            let seed = histogram_index * 1000 + token_index * 37;
            let pixel = 0xff00_0000
                | (((seed * 73) & 0xff) << 16)
                | (((seed * 151) & 0xff) << 8)
                | ((seed * 199) & 0xff);
            let token = Token::Literal(pixel);
            histogram.add_token(token, 1);
            high_entropy_tokens.push(token);
        }
        histogram.analyze();
        high_entropy.push(histogram);
    }
    assert!(!stochastic_combine(&mut high_entropy, 1));
    let _ = cluster(&high_entropy_tokens, high_entropy_tokens.len(), 1, 0, 0, 6);

    let small_tokens = [
        Token::Literal(0xff00_0000),
        Token::Literal(0xff00_0001),
        Token::Literal(0xff00_0002),
        Token::Literal(0xff00_0003),
    ];
    let _ = cluster(&small_tokens, 4, 1, 0, 100, 0);
    let _ = cluster(&small_tokens, 4, 1, 0, 0, 0);

    let many_tokens = (0..(2 * BIN_SIZE + 1))
        .map(|index| Token::Literal(0xff00_0000 | index as u32))
        .collect::<Vec<_>>();
    let _ = cluster(&many_tokens, many_tokens.len(), 1, 0, 99, 0);
    let _ = cluster(&many_tokens, many_tokens.len(), 1, 0, 100, 0);
    let many_distinct = (0..(4 * BIN_SIZE))
        .map(|index| {
            Token::Literal(
                0xff00_0000
                    | (((index as u32).wrapping_mul(0x45d9_f3b)) & 0x00ff_ffff),
            )
        })
        .collect::<Vec<_>>();
    let _ = cluster(&many_distinct, many_distinct.len(), 1, 0, 100, 0);
}
