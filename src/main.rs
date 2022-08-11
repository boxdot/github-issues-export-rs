use std::io::Write;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::{fmt, io};

use anyhow::{anyhow, bail};
use argh::FromArgs;
use futures::StreamExt;
use handlebars::Handlebars;
use headers::{authorization::Bearer, Authorization, ContentType, HeaderMapExt, UserAgent};
use hyper::{
    client::{Client, HttpConnector},
    Body, Request,
};
use hyper_tls::HttpsConnector;
use serde::Deserialize;
use tracing::{debug, info};

mod model;
mod template;

#[derive(Clone)]
struct Github {
    client: Client<HttpsConnector<HttpConnector>>,
    user_agent: UserAgent,
    auth: Authorization<Bearer>,
}

const API_ENDPOINT: &str = "https://api.github.com";

impl Github {
    pub fn new(auth: Authorization<Bearer>) -> anyhow::Result<Self> {
        let user_agent = UserAgent::from_static(concat!(
            env!("CARGO_PKG_NAME"),
            "/",
            env!("CARGO_PKG_VERSION")
        ));
        let https = HttpsConnector::new();
        let client = hyper::Client::builder().build(https);
        Ok(Self {
            client,
            user_agent,
            auth,
        })
    }

    async fn get<T>(&self, endpoint: &str) -> anyhow::Result<T>
    where
        T: serde::de::DeserializeOwned,
    {
        let mut req = Request::get(endpoint);
        if let Some(headers) = req.headers_mut() {
            headers.typed_insert(self.user_agent.clone());
            headers.typed_insert(self.auth.clone());
            headers.typed_insert(ContentType::json());
        }
        let req = req.body(Body::empty())?;

        debug!(?req, "request");
        let resp = self.client.request(req).await?;
        let status = resp.status();
        let body_bytes = hyper::body::to_bytes(resp.into_body()).await?;

        if status.is_success() {
            Ok(serde_json::from_slice(&body_bytes)?)
        } else {
            bail!("request failed: {}", String::from_utf8_lossy(&body_bytes));
        }
    }

    async fn issue(
        &self,
        Query {
            username,
            repo,
            issue,
        }: &Query,
    ) -> anyhow::Result<model::Issue> {
        let issue = issue.expect("logic error: querying issue without issue number");
        self.get(&format!(
            "{API_ENDPOINT}/repos/{username}/{repo}/issues/{issue}",
        ))
        .await
    }

    async fn issues(
        &self,
        Query { username, repo, .. }: &Query,
        state: State,
    ) -> anyhow::Result<Vec<model::Issue>> {
        self.get(&format!(
            "{API_ENDPOINT}/repos/{username}/{repo}/issues?state={state}",
        ))
        .await
    }
}

fn mkdir(path: impl AsRef<Path>) -> io::Result<()> {
    if let Err(e) = std::fs::create_dir(path) {
        match e.kind() {
            std::io::ErrorKind::AlreadyExists => (),
            _ => {
                return Err(e);
            }
        }
    };
    Ok(())
}

fn issue_to_filename(path: impl AsRef<Path>, issue: &model::Issue) -> String {
    format!(
        "{}/{:03}-{}.md",
        path.as_ref().display(),
        issue.number,
        slug::slugify(&issue.title),
    )
}

fn serialize(
    path: impl AsRef<Path>,
    hb: &mut Handlebars,
    data: &model::IssueWithComments,
) -> anyhow::Result<()> {
    let md = hb.render("issue", &data)?;
    let filename = issue_to_filename(path, &data.issue);
    let mut f = std::fs::File::create(&filename)?;
    info!("Writing name {}", filename);
    f.write_all(md.as_bytes())?;
    Ok(())
}

#[derive(Debug, Deserialize)]
enum State {
    Open,
    Closed,
    All,
}

impl FromStr for State {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "open" => Self::Open,
            "closed" => Self::Closed,
            "all" => Self::All,
            _ => bail!("unknown state: {s}"),
        })
    }
}

impl fmt::Display for State {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            State::Open => write!(f, "open"),
            State::Closed => write!(f, "closed"),
            State::All => write!(f, "all"),
        }
    }
}

/// Export issues from GitHub into markdown files.
///
/// Requires environment variable GITHUB_TOKEN (in environment or .env file)
#[derive(Debug, FromArgs)]
struct Args {
    /// output directory [default: ./md]
    #[argh(option, short = 'p', default = "PathBuf::from(\"./md\")")]
    path: PathBuf,
    /// fetch issues that are open, closed, or both [default: open]
    #[argh(option, short = 's', default = "State::Open")]
    state: State,
    /// query of the form: username/repo[#issue_number]
    #[argh(positional)]
    query: Query,
}

#[derive(Debug)]
struct Query {
    username: String,
    repo: String,
    issue: Option<usize>,
}

impl FromStr for Query {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (username, repo) = s
            .split_once('/')
            .ok_or_else(|| anyhow!("invalid query: {s}"))?;
        let (repo, issue) = repo
            .split_once('#')
            .map(|(repo, issue)| (repo, Some(issue)))
            .unwrap_or_else(|| (repo, None));
        let issue = issue
            .map(|s| {
                s.parse()
                    .map_err(|_| anyhow!("failed to parse issue {s} as integer"))
            })
            .transpose()?;
        Ok(Self {
            username: username.to_string(),
            repo: repo.to_string(),
            issue,
        })
    }
}

const MAX_PARALLEL_FETCHES: usize = 8;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = init();
    let token = dotenv::var("GITHUB_TOKEN")
        .map_err(|_| anyhow!("missing obligatory environment variable GITHUB_TOKEN"))?;

    let auth = Authorization::bearer(&token)?;
    let github = Github::new(auth)?;

    let mut reg = Handlebars::new();
    reg.register_template_string("issue", template::TEMPLATE)?;

    let issues: Vec<model::Issue> = if args.query.issue.is_some() {
        let issue = github.issue(&args.query).await?;
        vec![issue]
    } else {
        github.issues(&args.query, args.state).await?
    };

    let mut issues = futures::stream::iter(issues.into_iter().map(|issue| {
        let github = github.clone();
        async move {
            let comments: Vec<model::Comment> = github.get(&issue.comments_url).await?;
            Ok::<_, anyhow::Error>(model::IssueWithComments { issue, comments })
        }
    }))
    .buffer_unordered(MAX_PARALLEL_FETCHES);

    mkdir(&args.path)?;

    while let Some(data) = issues.next().await {
        serialize(&args.path, &mut reg, &data?)?;
    }

    Ok(())
}

fn init() -> Args {
    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "info")
    }
    tracing_subscriber::fmt::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();
    argh::from_env()
}
