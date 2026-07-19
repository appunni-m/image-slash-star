//! Exact libwebp-compatible VP8 4×4 intra predictions.

#![allow(dead_code)]

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub(super) enum Intra4Mode {
    Dc = 0,
    TrueMotion = 1,
    Vertical = 2,
    Horizontal = 3,
    DownRight = 4,
    VerticalRight = 5,
    DownLeft = 6,
    VerticalLeft = 7,
    HorizontalDown = 8,
    HorizontalUp = 9,
}

fn average_two(a: u8, b: u8) -> u8 {
    ((u16::from(a) + u16::from(b) + 1) >> 1) as u8
}

fn average_three(a: u8, b: u8, c: u8) -> u8 {
    ((u16::from(a) + 2 * u16::from(b) + u16::from(c) + 2) >> 2) as u8
}

fn set(output: &mut [u8; 16], column: usize, row: usize, value: u8) {
    output[row * 4 + column] = value;
}

pub(super) fn predict(mode: Intra4Mode, top: &[u8; 8], left: &[u8; 4], top_left: u8) -> [u8; 16] {
    let [a, b, c, d, e, f, g, h] = *top;
    let [i, j, k, l] = *left;
    let x = top_left;
    let mut output = [0; 16];

    match mode {
        Intra4Mode::Dc => {
            let dc = (4
                + top[..4].iter().map(|&value| u32::from(value)).sum::<u32>()
                + left.iter().map(|&value| u32::from(value)).sum::<u32>())
                >> 3;
            output.fill(dc as u8);
        }
        Intra4Mode::TrueMotion => {
            for row in 0..4 {
                for column in 0..4 {
                    set(
                        &mut output,
                        column,
                        row,
                        (i16::from(top[column]) + i16::from(left[row]) - i16::from(x)).clamp(0, 255)
                            as u8,
                    );
                }
            }
        }
        Intra4Mode::Vertical => {
            let row = [
                average_three(x, a, b),
                average_three(a, b, c),
                average_three(b, c, d),
                average_three(c, d, e),
            ];
            for row_index in 0..4 {
                output[row_index * 4..row_index * 4 + 4].copy_from_slice(&row);
            }
        }
        Intra4Mode::Horizontal => {
            let rows = [
                average_three(x, i, j),
                average_three(i, j, k),
                average_three(j, k, l),
                average_three(k, l, l),
            ];
            for (row, value) in rows.into_iter().enumerate() {
                output[row * 4..row * 4 + 4].fill(value);
            }
        }
        Intra4Mode::DownRight => {
            let references = [l, k, j, i, x, a, b, c, d];
            for row in 0..4 {
                for column in 0..4 {
                    let center = 4 + column as isize - row as isize;
                    set(
                        &mut output,
                        column,
                        row,
                        average_three(
                            references[(center - 1) as usize],
                            references[center as usize],
                            references[(center + 1) as usize],
                        ),
                    );
                }
            }
        }
        Intra4Mode::VerticalRight => {
            output = [
                average_two(x, a),
                average_two(a, b),
                average_two(b, c),
                average_two(c, d),
                average_three(i, x, a),
                average_three(x, a, b),
                average_three(a, b, c),
                average_three(b, c, d),
                average_three(j, i, x),
                average_two(x, a),
                average_two(a, b),
                average_two(b, c),
                average_three(k, j, i),
                average_three(i, x, a),
                average_three(x, a, b),
                average_three(a, b, c),
            ];
        }
        Intra4Mode::DownLeft => {
            let references = [a, b, c, d, e, f, g, h, h];
            for row in 0..4 {
                for column in 0..4 {
                    let index = row + column;
                    set(
                        &mut output,
                        column,
                        row,
                        average_three(
                            references[index],
                            references[index + 1],
                            references[index + 2],
                        ),
                    );
                }
            }
        }
        Intra4Mode::VerticalLeft => {
            output = [
                average_two(a, b),
                average_two(b, c),
                average_two(c, d),
                average_two(d, e),
                average_three(a, b, c),
                average_three(b, c, d),
                average_three(c, d, e),
                average_three(d, e, f),
                average_two(b, c),
                average_two(c, d),
                average_two(d, e),
                average_three(e, f, g),
                average_three(b, c, d),
                average_three(c, d, e),
                average_three(d, e, f),
                average_three(f, g, h),
            ];
        }
        Intra4Mode::HorizontalDown => {
            output = [
                average_two(i, x),
                average_three(i, x, a),
                average_three(x, a, b),
                average_three(a, b, c),
                average_two(j, i),
                average_three(j, i, x),
                average_two(i, x),
                average_three(i, x, a),
                average_two(k, j),
                average_three(k, j, i),
                average_two(j, i),
                average_three(j, i, x),
                average_two(l, k),
                average_three(l, k, j),
                average_two(k, j),
                average_three(k, j, i),
            ];
        }
        Intra4Mode::HorizontalUp => {
            output = [
                average_two(i, j),
                average_three(i, j, k),
                average_two(j, k),
                average_three(j, k, l),
                average_two(j, k),
                average_three(j, k, l),
                average_two(k, l),
                average_three(k, l, l),
                average_two(k, l),
                average_three(k, l, l),
                l,
                l,
                l,
                l,
                l,
                l,
            ];
        }
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn predictions_match_libwebp_1_6_0() {
        let top = [100, 110, 120, 130, 140, 150, 160, 170];
        let left = [80, 70, 60, 50];
        let expected = [
            [
                90, 90, 90, 90, 90, 90, 90, 90, 90, 90, 90, 90, 90, 90, 90, 90,
            ],
            [
                90, 100, 110, 120, 80, 90, 100, 110, 70, 80, 90, 100, 60, 70, 80, 90,
            ],
            [
                100, 110, 120, 130, 100, 110, 120, 130, 100, 110, 120, 130, 100, 110, 120, 130,
            ],
            [
                80, 80, 80, 80, 70, 70, 70, 70, 60, 60, 60, 60, 53, 53, 53, 53,
            ],
            [
                90, 100, 110, 120, 80, 90, 100, 110, 70, 80, 90, 100, 60, 70, 80, 90,
            ],
            [
                95, 105, 115, 125, 90, 100, 110, 120, 80, 95, 105, 115, 70, 90, 100, 110,
            ],
            [
                110, 120, 130, 140, 120, 130, 140, 150, 130, 140, 150, 160, 140, 150, 160, 168,
            ],
            [
                105, 115, 125, 135, 110, 120, 130, 140, 115, 125, 135, 150, 120, 130, 140, 160,
            ],
            [
                85, 90, 100, 110, 75, 80, 85, 90, 65, 70, 75, 80, 55, 60, 65, 70,
            ],
            [
                75, 70, 65, 60, 65, 60, 55, 53, 55, 53, 50, 50, 50, 50, 50, 50,
            ],
        ];
        let modes = [
            Intra4Mode::Dc,
            Intra4Mode::TrueMotion,
            Intra4Mode::Vertical,
            Intra4Mode::Horizontal,
            Intra4Mode::DownRight,
            Intra4Mode::VerticalRight,
            Intra4Mode::DownLeft,
            Intra4Mode::VerticalLeft,
            Intra4Mode::HorizontalDown,
            Intra4Mode::HorizontalUp,
        ];
        for (index, mode) in modes.into_iter().enumerate() {
            assert_eq!(
                predict(mode, &top, &left, 90),
                expected[index],
                "mode {index}"
            );
        }
    }
}
