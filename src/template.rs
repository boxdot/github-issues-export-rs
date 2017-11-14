pub const TEMPLATE: &'static str = r#"# [{{issue.title}}]({{issue.html_url}})

> state: **{{issue.state}}** opened by: **{{issue.user.login}}** on: **{{issue.created_at}}**

{{{issue.body}}}

### Comments
{{#each comments}}
---
> from: [**{{user.login}}**]({{html_url}}) on: **{{created_at}}**

{{{body}}}
{{~/each}}
"#;
