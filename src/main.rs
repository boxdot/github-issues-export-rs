extern crate docopt;
#[macro_use]
extern crate error_chain;
extern crate futures;
extern crate handlebars;
extern crate hyper;
extern crate hyper_tls;
extern crate native_tls;
extern crate serde;
extern crate serde_json;
#[macro_use]
extern crate serde_derive;
extern crate slug;
extern crate tokio_core;

use docopt::Docopt;
use futures::Stream;
use futures::{future, Future};
use handlebars::Handlebars;
use hyper::header::{Authorization, ContentLength, ContentType, UserAgent};
use hyper::{Client, Method, Request, Uri};

use std::fmt;
use std::io::Write;
use std::str::FromStr;

mod model;
mod template;

mod errors {
    error_chain!{
        errors {
            Request(t: String) {
                description("invalid request name")
                display("request failed: '{}'", t)
            }
        }
        foreign_links {
            Docopt(::docopt::Error);
            Io(::std::io::Error);
            Hyper(::hyper::Error);
            HbTemplate(::handlebars::TemplateError);
            HbRender(::handlebars::RenderError);
            NativeTls(::native_tls::Error);
            Json(::serde_json::Error);
            Utf8(::std::str::Utf8Error);
        }
    }
}

use errors::*;

struct Github {
    client: Client<hyper_tls::HttpsConnector<hyper::client::HttpConnector>>,
    user_agent: UserAgent,
    token: Authorization<String>,
}

impl Github {
    const API_ENDPOINT: &'static str = "https://api.github.com";

    pub fn new(
        handle: &tokio_core::reactor::Handle,
        agent: UserAgent,
        token: &str,
    ) -> Result<Self> {
        Ok(Self {
            client: hyper::Client::configure()
                .connector(hyper_tls::HttpsConnector::new(4, handle)?)
                .build(handle),
            user_agent: agent,
            token: Authorization(format!("token {}", token)),
        })
    }

    pub fn get<T>(&self, endpoint: &str) -> impl Future<Item = T, Error = Error>
    where
        T: serde::de::DeserializeOwned,
    {
        let url = Uri::from_str(endpoint).expect("Could not parse uri");
        let mut req = Request::new(Method::Get, url);
        req.headers_mut().set(self.user_agent.clone());
        req.headers_mut().set(self.token.clone());
        req.headers_mut().set(ContentType::json());
        req.headers_mut().set(ContentLength(0));
        let resp = self.client.request(req);
        resp.map_err(Error::from).and_then(|resp| {
            let status_code = resp.status();
            let body = resp.body().concat2().from_err();
            body.and_then(move |chunk| {
                if !status_code.is_success() {
                    let resp = String::from(::std::str::from_utf8(&chunk)?);
                    Err(ErrorKind::Request(resp).into())
                } else {
                    let value: T = ::serde_json::from_slice(&chunk)
                        .chain_err(|| "Could not parse response from server")?;
                    Ok(value)
                }
            })
        })
    }

    pub fn issue(
        &self,
        user: &str,
        repo: &str,
        number: usize,
    ) -> impl Future<Item = model::Issue, Error = errors::Error> {
        self.get(&format!(
            "{}/repos/{owner}/{repo}/issues/{number}",
            Self::API_ENDPOINT,
            owner = user,
            repo = repo,
            number = number
        ))
    }

    pub fn issues(
        &self,
        user: &str,
        repo: &str,
        state: &State,
    ) -> impl Future<Item = Vec<model::Issue>, Error = errors::Error> {
        self.get(&format!(
            "{}/repos/{}/{}/issues?state={}",
            Self::API_ENDPOINT,
            user,
            repo,
            state
        ))
    }
}

fn mkdir(path: &str) -> Result<()> {
    if let Err(err) = std::fs::create_dir(path) {
        match err.kind() {
            std::io::ErrorKind::AlreadyExists => (),
            _ => {
                return Err(Error::from(err));
            }
        }
    };
    Ok(())
}

fn issue_to_filename(path: &str, issue: &model::Issue) -> String {
    format!(
        "{}/{:03}-{}.md",
        path,
        issue.number,
        slug::slugify(&issue.title),
    )
}

fn serialize(path: &str, hb: &mut Handlebars, data: &model::IssueWithComments) -> Result<()> {
    let md = hb.render("issue", &data)?;
    let filename = issue_to_filename(path, &data.issue);
    let mut f = std::fs::File::create(&filename)?;
    println!("Writing name {}", filename);
    f.write_all(md.as_bytes())?;
    Ok(())
}

const USAGE: &'static str = r#"
Export issues from GitHub into markdown files.

Usage:
  github-issues-export [options] <query>
  github-issues-export (-h | --help)
  github-issues-export --version

<query> is of the form: username/repo[#issue_number].

Environment variables:
  GITHUB_TOKEN      Authorization token for GitHub.

Options:
  -h --help                         Show this screen.
  --version                         Show version.
  -p --path=<directory>             Output directory [default: ./md].
  -s --state=<open|closed|all>      Fetch issues that are open, closed, or
                                    both [default: open].
"#;

#[derive(Debug, Deserialize)]
enum State {
    Open,
    Closed,
    All,
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

#[derive(Debug, Deserialize)]
struct Args {
    flag_version: bool,
    #[serde(skip)]
    env_token: String,
    arg_query: String,
    #[serde(skip)]
    arg_username: String,
    #[serde(skip)]
    arg_repo: String,
    #[serde(skip)]
    arg_issue: Option<usize>,
    flag_path: String,
    flag_state: State,
}

fn parse_args() -> Args {
    let mut args: Args = Docopt::new(USAGE)
        .and_then(|d| d.deserialize())
        .unwrap_or_else(|e| e.exit());

    if args.flag_version {
        println!("{} {}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
        std::process::exit(0);
    }

    args.env_token = std::env::var("GITHUB_TOKEN").unwrap_or_else(|_| {
        eprintln!("Missing obligatory environment variable GITHUB_TOKEN");
        std::process::exit(1);
    });

    {
        let parts: Vec<_> = args.arg_query.split("/").collect();
        if parts.len() != 2 {
            eprintln!("Wrong argument: {}.\n\n{}", args.arg_query, USAGE);
            std::process::exit(1);
        }
        args.arg_username = String::from(parts[0]);
        let parts: Vec<_> = parts[1].split("#").collect();
        if parts.len() == 1 {
            args.arg_repo = String::from(parts[0]);
        } else if parts.len() == 2 {
            args.arg_repo = String::from(parts[0]);
            args.arg_issue = match parts[1].parse::<usize>() {
                Ok(value) => Some(value),
                Err(_) => {
                    eprintln!("Wrong argument: {}.\n\n{}", args.arg_query, USAGE);
                    std::process::exit(1);
                }
            }
        } else {
            eprintln!("Wrong argument: {}.\n\n{}", args.arg_query, USAGE);
            std::process::exit(1);
        }
    }

    args
}

fn run() -> Result<()> {
    let args = parse_args();

    let mut core = tokio_core::reactor::Core::new()?;
    let handle = core.handle();

    let github = Github::new(
        &handle,
        UserAgent::new(format!(
            "{}/{}",
            env!("CARGO_PKG_NAME"),
            env!("CARGO_PKG_VERSION")
        )),
        &args.env_token,
    )?;

    let fut_issues: Box<Future<Item = _, Error = _>> = match args.arg_issue {
        Some(issue_number) => Box::new(
            github
                .issue(&args.arg_username, &args.arg_repo, issue_number)
                .map(|issue| vec![issue]),
        ),
        None => Box::new(github.issues(&args.arg_username, &args.arg_repo, &args.flag_state)),
    };

    let fut_issues_with_comments = fut_issues.and_then(|issues| {
        future::join_all(issues.into_iter().map(|issue| {
            let comments_url = issue.comments_url.clone();
            Future::join(
                future::ok(issue),
                github.get::<Vec<model::Comment>>(&comments_url),
            )
        }))
    });
    let fut_data = fut_issues_with_comments.map(|issues| {
        issues
            .into_iter()
            .map(|(issue, comments)| model::IssueWithComments {
                issue: issue,
                comments: comments,
            })
    });

    let issues = core.run(fut_data)?;

    let mut reg = Handlebars::new();
    reg.register_template_string("issue", template::TEMPLATE)?;

    mkdir(&args.flag_path)?;
    for data in issues.into_iter() {
        serialize(&args.flag_path, &mut reg, &data)?;
    }

    Ok(())
}

fn main() {
    if let Err(ref e) = run() {
        use std::io::Write;
        let stderr = &mut ::std::io::stderr();
        let errmsg = "Error writing to stderr";

        writeln!(stderr, "Error: {}", e).expect(errmsg);
        for e in e.iter().skip(1) {
            writeln!(stderr, "Caused by: {}", e).expect(errmsg);
        }

        ::std::process::exit(1);
    }
}
