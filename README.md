# gh2md
Export GitHub issues to markdown files.

## Usage:

```
export GITHUB_TOKEN=<your_secret_token>
gh2md username/repo[#issue_number]
```

This will export all open issues from GitHub repository `username/repo` into directory `./md` 
putting each issue into a separate file including its comments.
