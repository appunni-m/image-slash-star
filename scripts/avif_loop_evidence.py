"""Read independently collected complete-file native repetition evidence.

This is a provenance lookup, not a second implementation of AVIF edit lists.
Unknown inputs require a native observation before reference generation.
"""

from __future__ import annotations

import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
INDEX = Path("tests/fixtures/outputs/avif_loops/index.json")


class LoopEvidenceError(RuntimeError):
    """A required independent loop observation is missing or inconsistent."""


def normalized_loop(repetitions):
    if type(repetitions) is not int or not -2 <= repetitions <= (1 << 31) - 1:
        raise LoopEvidenceError("invalid native signed repetition count")
    return None if repetitions == -2 else 0 if repetitions == -1 else repetitions + 1


def sequence_loop_evidence(data):
    try:
        raw = (ROOT / INDEX).read_bytes()
        index = json.loads(raw)
        if index["schema"] != "image-slash-star/avif-loop-oracle@1" or index["origin"] != "libavif.avifDecoder.repetitionCount":
            raise LoopEvidenceError("AVIF loop oracle schema or origin differs")
        if index["source"]["commit"] != "6543b22b5bc706c53f038a16fe515f921556d9b3" or index["oracle"]["libavif"] != "1.4.1":
            raise LoopEvidenceError("AVIF loop native source differs from pin")
        digest = hashlib.sha256(data).hexdigest()
        cases = [case for case in index["cases"] if case["input_sha256"] == digest]
        if len(cases) != 1:
            raise LoopEvidenceError(f"complete AVIF input needs a unique native loop observation: {digest}")
        case, = cases
        if case["input_bytes"] != len(data) or not case["repeat_equal"] or case["native"]["parse_result"] != 0 or case["pillow"]["status"] != "ok":
            raise LoopEvidenceError("AVIF loop observation is not a repeated successful decode")
        source_path = Path(case["input_path"])
        if source_path.is_absolute() or ".." in source_path.parts or (ROOT / INDEX.parent / source_path).read_bytes() != data:
            raise LoopEvidenceError("AVIF loop witness bytes differ from generator input")
        repetitions = case["native"]["repetition_count"]
        return normalized_loop(repetitions), {
            "index_path": INDEX.as_posix(), "index_sha256": hashlib.sha256(raw).hexdigest(),
            "case": case["name"], "input_sha256": digest, "repetition_count": repetitions,
        }
    except (KeyError, OSError, TypeError, ValueError) as error:
        raise LoopEvidenceError(f"invalid AVIF loop evidence: {error}") from error


def validate_loop_evidence(sequence, input_sha256):
    evidence = sequence.get("loop_evidence")
    if not isinstance(evidence, dict) or evidence.get("index_path") != INDEX.as_posix():
        raise LoopEvidenceError("AVIF sequence lacks its native loop evidence")
    if not input_sha256 or evidence.get("input_sha256") != input_sha256:
        raise LoopEvidenceError("AVIF loop evidence does not match the enclosing complete input")
    index = json.loads((ROOT / INDEX).read_bytes())
    matches = [case for case in index["cases"] if case["name"] == evidence.get("case")]
    if len(matches) != 1:
        raise LoopEvidenceError("AVIF loop case identity is not unique")
    case, = matches
    path = Path(case["input_path"])
    if path.is_absolute() or ".." in path.parts:
        raise LoopEvidenceError("AVIF loop witness path is invalid")
    value, actual_evidence = sequence_loop_evidence((ROOT / INDEX.parent / path).read_bytes())
    if actual_evidence != evidence or sequence.get("loop_count") != value or sequence.get("loop_origin") != "independent_implementation":
        raise LoopEvidenceError("AVIF sequence loop value or provenance differs from native observation")
