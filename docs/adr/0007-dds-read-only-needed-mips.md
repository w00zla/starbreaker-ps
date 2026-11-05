# 0007. Read only the DDS mip siblings the requested mip needs

- Status: accepted
- Date: 2026-11-04
- Recorded: 2026-11-05
- Scope: starbreaker-dds, texture loaders in starbreaker-3d

## Context

`DdsFile::from_split` read every split sibling (`.dds.8` down to `.dds.1`) of a texture, although icon exports use a
reduced mip level (`TEX2`, quarter resolution). The full level is about 75 % of the bytes, the next about 19 %. The P4k
read cost dominated an export.

## Decision

New `DdsFile::from_split_min_mip(base, reader, min_mip)`; `from_split` calls it with 0. A new reader trait method
`sibling_size(suffix)` tells which siblings exist without reading them (filesystem: metadata; P4k: uncompressed size from
the central directory). Sibling i is mip i counted downward and the base file holds the rest; alpha siblings (`.8a` to `.1a`)
work likewise and `.a` is always read. Only siblings at or above `min(min_mip, last colour level, last alpha level)` are
read; if a sibling exists but cannot be read, the code falls back to the full read. The three loaders (diffuse, normal,
roughness) pass their `mip_level`.

Measured on 60 entities at mip 2: bytes read from the P4k 2621 -> 426 MB (-84 %), user time -25 %, wall -16 %, maximum
RSS -300 MB, and all 1461 output files byte-identical to the old binary. A scan of all 46 891 DDS in the P4k confirmed the
sibling numbering, cubemaps and alpha included.

## Alternatives

- Computing the sibling count from the DDS header alone: not possible, `bNumPersistentMips` is practically always 0 and DX10 texture arrays give wrong face counts.
- `TEX3` instead of `TEX2` (about a quarter of the texture bytes): not taken, possibly less sharp.

## Consequences

Callers that want all levels use `from_split` unchanged. Every new reader implementation must provide `sibling_size`.

## References

`38e26bc3`.
