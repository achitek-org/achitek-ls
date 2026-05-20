# Changelog

## [Unreleased]

## [0.3.0](https://github.com/achitek-org/achitek-ls/compare/achitekfile-v0.2.0...achitekfile-v0.3.0)

### ⛰️ Features


- Introduce workspace - ([757e661](https://github.com/achitek-org/achitek-ls/commit/757e6617cc7402b96fb11737b778f9b42bcf7799))

### 🚜 Refactor


- *(achitekfile)* Lower syntax into model - ([1eba206](https://github.com/achitek-org/achitek-ls/commit/1eba206d577b525654701f8ef3131aa7a0328ad1))
- Move shared utils and structs to achitek-source crate - ([9775e3e](https://github.com/achitek-org/achitek-ls/commit/9775e3e183b76cdf77b057d704d1a9e0ca24b5c7))

### ⚙️ Miscellaneous Tasks


- *(achitekfile)* Rename parse_tree - ([f24c6e0](https://github.com/achitek-org/achitek-ls/commit/f24c6e010379488173aaadacb85e4a04eda3dd83))
- Reference achitekfile locally and small refactor in parser - ([927c20e](https://github.com/achitek-org/achitek-ls/commit/927c20edddf28f042bc4bcd99658cea3e3e7ed31))
- Update workspace - ([4d67daf](https://github.com/achitek-org/achitek-ls/commit/4d67daf8ded392abaa60ec332a2ee10f3fa4e84b))


## [0.2.0](https://github.com/achitek-org/achitekfile/compare/v0.1.0...v0.2.0)

### ⛰️ Features


- Implement full diagnostics suite ([#16](https://github.com/achitek-org/achitekfile/pull/16)) - ([db6ec0a](https://github.com/achitek-org/achitekfile/commit/db6ec0a100613426b12699c422d0c040b8d91392))
- Add semantic diagnostics ([#15](https://github.com/achitek-org/achitekfile/pull/15)) - ([9461a43](https://github.com/achitek-org/achitekfile/commit/9461a433a1fa04dac294b0f4facf2581b7b8b28e))
- Implement Achitekfile and ValidAchitekfile ([#14](https://github.com/achitek-org/achitekfile/pull/14)) - ([3b10598](https://github.com/achitek-org/achitekfile/commit/3b1059813026ce083c83f7b3041f2a32395d5da6))
- Implement analyze api ([#13](https://github.com/achitek-org/achitekfile/pull/13)) - ([b90ce0b](https://github.com/achitek-org/achitekfile/commit/b90ce0b69a70f586e03813998c6ffb8691a014c2))
- Add getters for diagnostic code kind, severity and as_str api - ([574470d](https://github.com/achitek-org/achitekfile/commit/574470dc4139cc669bb98f8d9d6d7c799476ecf3))
- Define source range and diagnostic primitives ([#12](https://github.com/achitek-org/achitekfile/pull/12)) - ([f10f9ed](https://github.com/achitek-org/achitekfile/commit/f10f9ed4bc5e28a580e27436b7cbb38e7fe89883))

### 🐛 Bug Fixes


- Avoid false-positive analysis diagnostics ([#18](https://github.com/achitek-org/achitekfile/pull/18)) - ([a3008c3](https://github.com/achitek-org/achitekfile/commit/a3008c3ad5d49356d1b576b5f4e9ee8eaa5b9e47))

### 📚 Documentation


- Tiny doc clean up - ([2bbeccd](https://github.com/achitek-org/achitekfile/commit/2bbeccd0931fb09ba7c0a5626607e8f0f8255d40))

### ⚙️ Miscellaneous Tasks


- Addresses M-FIRST-DOC-SENTENCE - ([8fe7404](https://github.com/achitek-org/achitekfile/commit/8fe740446e4a0751244ce6823053a9c73df67e92))
- Addresses M-ERRORS-CANONICAL-STRUCTS - ([d8fda39](https://github.com/achitek-org/achitekfile/commit/d8fda3994e3b6b921cce04cc676ce66b77d026a0))
- Addresses M-DOCUMENTED-MAGIC - ([62a17f2](https://github.com/achitek-org/achitekfile/commit/62a17f21aa0f3c1438b9037c9dafcfefcb0da63f))
- Addresses M-PANIC-ON-BUG - ([ac8d494](https://github.com/achitek-org/achitekfile/commit/ac8d4943a66cb54d0ba38d7f7f860dda66b5f34d))
- Add missing '>' at the end of email provided for author - ([54d9157](https://github.com/achitek-org/achitekfile/commit/54d91577e6b40bdb3c35729ba5b6bf5121dbf1d4))
- Fmt - ([055a6b9](https://github.com/achitek-org/achitekfile/commit/055a6b9ec94e83377fe9522f555f9768e4a8bcfe))
- Address C-LINK - ([8906b67](https://github.com/achitek-org/achitekfile/commit/8906b6741f0fe1ee122abedf99d70156348407f8))
- Address C-FAILURE - ([cc51b52](https://github.com/achitek-org/achitekfile/commit/cc51b52bf0006a89490e775c948b2a29f745c065))
- Address C-QUESTION-MARK - ([8bfe4f3](https://github.com/achitek-org/achitekfile/commit/8bfe4f3e12bd177cabb01675c823c4c5ee6d7f5f))
- Address C-EXAMPLE - ([996e94b](https://github.com/achitek-org/achitekfile/commit/996e94b87a8ace35a8248e742df28a23667b68ce))
- Loosen SortError trait bounds - ([4a56ad8](https://github.com/achitek-org/achitekfile/commit/4a56ad835714593600fbb0d969b5467953dd94b2))
- Addresses C-SERDE - ([359c186](https://github.com/achitek-org/achitekfile/commit/359c186735d2d09e19b03d05b058b62e364fff4f))
- Update from_str references - ([0a51a8d](https://github.com/achitek-org/achitekfile/commit/0a51a8df8a63f7ac120b153024909621347d4d8d))
- Address C-CONV-TRAITS - ([48155c9](https://github.com/achitek-org/achitekfile/commit/48155c93ab01661e1aafbf476df4c79483cf941d))
- Address C-COMMON-TRAITS - ([39da236](https://github.com/achitek-org/achitekfile/commit/39da236751e1d77a8f12dfbfb1502fc14b7881db))
- Rename DAGAsAdjacencyList to AdjacencyList - ([1e2351a](https://github.com/achitek-org/achitekfile/commit/1e2351a615273584801d05656dca1592c5e1efcb))
- Address C-GETTER - ([e733733](https://github.com/achitek-org/achitekfile/commit/e73373366e083a4927b77dbbc0b357777bcd6347))
- Rename from_str to parse_tree - ([b7e7d5c](https://github.com/achitek-org/achitekfile/commit/b7e7d5c7bf1870babe6eefc3f81df4865159f4c6))
- Addresses C-METADATA - ([d82d259](https://github.com/achitek-org/achitekfile/commit/d82d259169727fba3c0d45d10478f467cc1a81f1))


## [0.1.0]

### ⛰️ Features


- Initial implementation - ([782b237](https://github.com/achitek-org/achitekfile/commit/782b237486687c361d12668093ae0f301f080c98))

### 🐛 Bug Fixes


- Release workflow - ([22d3696](https://github.com/achitek-org/achitekfile/commit/22d36964efe16bb2dd966c5560cec9190bbae890))

### ⚙️ Miscellaneous Tasks


- Add ci and release workflow - ([afc2490](https://github.com/achitek-org/achitekfile/commit/afc2490deb8352c936d6b3fa715707b9f34c6293))
- Clean up - ([f179509](https://github.com/achitek-org/achitekfile/commit/f179509f3a472eec1c3f0b3879103456a876db05))

