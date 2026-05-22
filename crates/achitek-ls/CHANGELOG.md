# Changelog

## [Unreleased]

## [0.2.0](https://github.com/achitek-org/achitek-ls/compare/achitek-ls-v0.1.1...achitek-ls-v0.2.0)

### ⛰️ Features


- *(ls)* Capture non terminated if blocks and raise warning if element is missing in choices - ([6fd0751](https://github.com/achitek-org/achitek-ls/commit/6fd0751d5295cc83cd79f8c1f7d1caa5fc7ebbfa))
- *(ls)* Add tera diagnostic styling & capture prompt variables in file paths - ([3bf5007](https://github.com/achitek-org/achitek-ls/commit/3bf50075bc0fe9efebd666a7315de30658e3a42a))
- *(ls)* Implement first pass at tera code actions - ([d947886](https://github.com/achitek-org/achitek-ls/commit/d94788683436003b19394d390155ad0fb07b6a0d))
- *(ls)* Implement rename - ([accc384](https://github.com/achitek-org/achitek-ls/commit/accc3840cbca98e85ec93662c26c3d4af55c0d14))
- *(ls)* Implemenet go-to-definition from tera template to achitekfile - ([51831e6](https://github.com/achitek-org/achitek-ls/commit/51831e6c6b25dfa7c7fe072cdfacdac74b3ed63c))
- Introduce workspace - ([757e661](https://github.com/achitek-org/achitek-ls/commit/757e6617cc7402b96fb11737b778f9b42bcf7799))

### 🐛 Bug Fixes


- *(ls)* Dismiss tera builtins as diagnostic errors - ([f935d87](https://github.com/achitek-org/achitek-ls/commit/f935d87875f870d1c7c4c7a111e32e4bb5c48240))

### 🚜 Refactor


- *(ls)* Remove code-actions - ([417ca00](https://github.com/achitek-org/achitek-ls/commit/417ca00a75499891b05cd74eb21655b78c525587))
- *(ls)* Split editor features into modules - ([c4d900e](https://github.com/achitek-org/achitek-ls/commit/c4d900ec837bc5d0d9fcff6e94adeed670e1b1b7))
- *(ls)* Use achitekfile parser types - ([ed0cfbd](https://github.com/achitek-org/achitek-ls/commit/ed0cfbdf711b413dac03127ff856d9c4a7e7a8b0))
- *(terafile)* Consolidate vendored grammar ownership - ([d386b4c](https://github.com/achitek-org/achitek-ls/commit/d386b4c44de641b69e80a77d587aa7b4ff02c3b5))

### 🧪 Testing


- *(ls)* Move inline rename tests to integration tests - ([e804847](https://github.com/achitek-org/achitek-ls/commit/e80484734c215103d43b7262dd9bead14a525c0a))
- *(ls)* Move inline references tests to integration tests - ([dc0eb55](https://github.com/achitek-org/achitek-ls/commit/dc0eb55d5b64083f417d23c94cc4efd4517210f5))
- *(ls)* Move inline did change tests to integration tests - ([872ae47](https://github.com/achitek-org/achitek-ls/commit/872ae471c7c6b105e66dfa634c6642755b4f0acd))
- *(ls)* Move inline did open tests to integration tests - ([f46b681](https://github.com/achitek-org/achitek-ls/commit/f46b68130cd436fdd72d2f6459713a042d8cd588))
- *(ls)* Move inline did close tests to integration tests - ([95bcfa8](https://github.com/achitek-org/achitek-ls/commit/95bcfa8c75bc89d4d9275f8ead48524cfaaf3ced))
- *(ls)* Move inline workspace symbol tests to integration tests - ([5882a7e](https://github.com/achitek-org/achitek-ls/commit/5882a7e665d3c65addf6b39557b1a29addf61680))
- *(ls)* Move inline definition tests to integration tests - ([23edd90](https://github.com/achitek-org/achitek-ls/commit/23edd90f6dff2eb494b9299a42b5816b719c6a32))
- *(ls)* Move inline prepare rename tests to integration tests - ([e7a7714](https://github.com/achitek-org/achitek-ls/commit/e7a7714e522f4cd8c7124ab5cb829eeebade9448))
- *(ls)* Move inline selection range tests to integration tests - ([4e854ee](https://github.com/achitek-org/achitek-ls/commit/4e854eeed1111c8414aff961063954eacd2e5d9c))
- *(ls)* Move inline formatting tests to integration tests - ([cd80e67](https://github.com/achitek-org/achitek-ls/commit/cd80e67322cfc7931cf8e3a32e625a3e680864f6))
- *(ls)* Move inline folding range tests to integration tests - ([876d006](https://github.com/achitek-org/achitek-ls/commit/876d0066e693b4c612fa2845291ccd0da1d8a6b3))
- *(ls)* Move inline document symbol tests to integration tests - ([62e5805](https://github.com/achitek-org/achitek-ls/commit/62e580519347297bc4941b7592dbcc98bbd37ce9))
- *(ls)* Move inline completion tests to integration tests - ([50b8676](https://github.com/achitek-org/achitek-ls/commit/50b8676d35a7cfd5e878443a00102a8f293cf77c))
- *(ls)* Move inline hover tests to integration tests - ([dd9e1a0](https://github.com/achitek-org/achitek-ls/commit/dd9e1a0240d90cc5838e7d1e87cd7bb7d63ff4dd))

### ⚙️ Miscellaneous Tasks


- *(achitekfile)* Rename parse_tree - ([f24c6e0](https://github.com/achitek-org/achitek-ls/commit/f24c6e010379488173aaadacb85e4a04eda3dd83))
- *(ls)* Format code - ([e95e11b](https://github.com/achitek-org/achitek-ls/commit/e95e11b33bfceb1d37bd3a426932513d805b8282))
- *(ls)* Clean up diagnostic publishing in handlers - ([3e028e2](https://github.com/achitek-org/achitek-ls/commit/3e028e296a0e27e196f8f99718d17150d5264398))
- *(ls)* Refactor hover and completion editor features into separate module - ([dec12af](https://github.com/achitek-org/achitek-ls/commit/dec12af2bc9c3932195b4a29bb2346345b59d495))
- *(ls)* Begin using achitefile analysis semantic model - ([df7c340](https://github.com/achitek-org/achitek-ls/commit/df7c34015e10752497b2d1f5de01060ae2327409))
- *(ls)* Implement project diagnostics - ([335c62e](https://github.com/achitek-org/achitek-ls/commit/335c62e430a6aa1656a5dd456bd0cebf4381e407))
- *(ls)* Implement ProjectContext that owns the repeated project lookup/source-loading behavior - ([8ac91a1](https://github.com/achitek-org/achitek-ls/commit/8ac91a127fbf517aa7ba82e3b9706058b0bbc1c2))
- Reference achitekfile locally and small refactor in parser - ([927c20e](https://github.com/achitek-org/achitek-ls/commit/927c20edddf28f042bc4bcd99658cea3e3e7ed31))
- Pass server state to request handlers - ([3ac50b0](https://github.com/achitek-org/achitek-ls/commit/3ac50b0abd47985f3141bdc2ecbb10c4ab89b0e6))
- Update workspace - ([4d67daf](https://github.com/achitek-org/achitek-ls/commit/4d67daf8ded392abaa60ec332a2ee10f3fa4e84b))


## [0.1.1](https://github.com/achitek-org/achitek-ls/compare/v0.1.0...v0.1.1)

### 🐛 Bug Fixes


- Default to stdio if communications channel not provided - ([c69e8d3](https://github.com/achitek-org/achitek-ls/commit/c69e8d3aa832da603dc6ed9a6588f0d812c8b658))

### ⚙️ Miscellaneous Tasks


- Lint - ([b1a5e2d](https://github.com/achitek-org/achitek-ls/commit/b1a5e2da3d419f8ed0cf37087a3e49fde040c0d6))
- Update github token for release-plz release - ([537e241](https://github.com/achitek-org/achitek-ls/commit/537e241f58aad789c60e4068c934feb3cf2e7964))


## [0.1.0]

### ⛰️ Features


- Implement server ([#9](https://github.com/achitek-org/achitek-ls/pull/9)) - ([dd6568c](https://github.com/achitek-org/achitek-ls/commit/dd6568c63a83177b827a4eb9a2f08fe7488205d7))
- Implement command line interface using lexopt ([#8](https://github.com/achitek-org/achitek-ls/pull/8)) - ([cd5167b](https://github.com/achitek-org/achitek-ls/commit/cd5167ba774346da7ae784b3f30c49d96a4714af))
- Initial Achitek LSP - ([a8148b3](https://github.com/achitek-org/achitek-ls/commit/a8148b31af3534c54f5a5dac6bbae2d6db9c9508))

### 📚 Documentation


- Improve documentation - ([6a5030f](https://github.com/achitek-org/achitek-ls/commit/6a5030f1cac4457054a75583ad202b11a1af6c5c))

### ⚙️ Miscellaneous Tasks


- Implement initial ci/cd automation ([#10](https://github.com/achitek-org/achitek-ls/pull/10)) - ([60527e0](https://github.com/achitek-org/achitek-ls/commit/60527e0ed5b91f5250cca9058300c8c43a4b279c))
- Addressing cargo clippy fix recommendations - ([1526709](https://github.com/achitek-org/achitek-ls/commit/1526709b772d4daf883e5da1b678ede4047cea7f))
- Refactor from workspace to single binary and lib structure ([#6](https://github.com/achitek-org/achitek-ls/pull/6)) - ([5493801](https://github.com/achitek-org/achitek-ls/commit/54938016edc3b2747753bf189604e975ad066341))
- Clean up - ([6567de0](https://github.com/achitek-org/achitek-ls/commit/6567de0ea724d91ff622e6e5a1f3fecda18705ed))
- Install deps - ([b2ee3b9](https://github.com/achitek-org/achitek-ls/commit/b2ee3b910a11d85488a5df15ce6bac5587fba298))

