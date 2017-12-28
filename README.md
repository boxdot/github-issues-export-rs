# github-issues-export [![Build Status](https://travis-ci.org/boxdot/github-issues-export-rs.svg?branch=master)](https://travis-ci.org/boxdot/github-issues-export-rs)
Export GitHub issues to markdown files.

## Usage:

```
export GITHUB_TOKEN=<your_secret_token>
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

