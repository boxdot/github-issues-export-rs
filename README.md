# github-issues-export

[![crates-badge]][crates-url]
[![docs-badge]][docs-url]
[![license-badge]][license]
[![ci-badge]][ci-url]

[crates-badge]: https://img.shields.io/crates/v/github-issues-export.svg
[crates-url]: https://crates.io/crates/github-issues-export
[docs-badge]: https://docs.rs/github-issues-export/badge.svg
[docs-url]: https://docs.rs/github-issues-export
[license-badge]: https://img.shields.io/crates/l/github-issues-export.svg
[license]: #license
[ci-badge]: https://github.com/boxdot/github-issues-export-rs/actions/workflows/rust.yml/badge.svg
[ci-url]:https://github.com/boxdot/github-issues-export-rs/actions/workflows/rust.yml

Export GitHub issues to markdown files.

## Usage

```shell
export GITHUB_TOKEN=<your_secret_token> # or put it in .env
github-issues-export username/repo[#issue_number]
```

This will export all open issues from GitHub repository `username/repo` into directory `./md`
putting each issue into a separate file including its comments.

## License

 * Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or
   http://www.apache.org/licenses/LICENSE-2.0)
 * MIT License ([LICENSE-MIT](LICENSE-MIT) or
   http://opensource.org/licenses/MIT)

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in this document by you, as defined in the Apache-2.0 license,
shall be dual licensed as above, without any additional terms or conditions.

