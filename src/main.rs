// ggl --- global git log
// Copyright (C) 2022  Honza Pokorny <honza@pokorny.ca>

// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.

use chrono;
use colored::*;
use dirs;
use git2;
use serde::Deserialize;
use std::fs;
use std::ops::Sub;
use std::path::{Path, PathBuf};
use std::str;
use structopt::StructOpt;

#[derive(StructOpt)]
struct Args {
    #[structopt(name = "until", long, short)]
    /// How far into the past should we go?  e.g. 2022-12-31; defaults to one week ago
    until: Option<String>,

    #[structopt(name = "fetch", long, short)]
    /// Run git fetch
    fetch: bool,

    #[structopt(name = "json", long, short)]
    /// Print JSON
    json: bool,

    #[structopt(name = "config", long, short)]
    /// Path to config file
    config: Option<PathBuf>,
}

#[derive(Debug, Deserialize)]
enum GglError {
    ConfigParserError(String),
    GitError(String),
    MissingConfigFile,
}

impl From<git2::Error> for GglError {
    fn from(err: git2::Error) -> Self {
        GglError::GitError(err.message().to_owned())
    }
}

impl From<serde_yaml::Error> for GglError {
    fn from(err: serde_yaml::Error) -> Self {
        GglError::ConfigParserError(format!("{}", err))
    }
}

#[derive(Debug, PartialEq, Deserialize)]
enum FilterType {
    Include,
    Reject,
}

#[derive(Debug, Deserialize)]
struct Filter {
    filter_type: FilterType,
    paths: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct Repository {
    name: String,
    path: String,
    remote: String,
    branch: String,
    fetch: bool,
    filters: Option<Vec<Filter>>,
}

#[derive(Debug, Deserialize)]
struct Config {
    root: String,
    repositories: Vec<Repository>,
}

#[derive(Debug)]
struct GlobalCommit {
    author: String,
    date: git2::Time,
    message: String,
    repo_name: String,
    sha: String,
}

fn load_config(path: PathBuf) -> Result<Config, GglError> {
    let contents = fs::read_to_string(path).unwrap();
    // TODO: Not sure why we can't return:
    //    serde_yaml::from_str(&contents)?;
    match serde_yaml::from_str(&contents) {
        Ok(c) => Ok(c),
        Err(e) => Err(GglError::ConfigParserError(format!("{}", e))),
    }
}

fn git_fetch(repo: &git2::Repository, r: &Repository) -> Result<(), git2::Error> {
    if !r.fetch {
        return Ok(());
    }

    println!("Fetching {} {}/{}", &r.name, &r.remote, &r.branch);
    repo.find_remote(&r.remote)?.fetch(&[&r.branch], None, None)
}

fn should_be_included(filters: &Vec<Filter>, changed_files: &Vec<PathBuf>) -> bool {
    for filter in filters {
        for filter_path in &filter.paths {
            for file in changed_files {
                if file.to_str().unwrap().contains(filter_path) {
                    match filter.filter_type {
                        FilterType::Include => {
                            return true;
                        }
                        FilterType::Reject => {
                            return false;
                        }
                    }
                }
            }
        }
    }
    false
}

fn collect_commits(
    config: &Config,
    fetch: bool,
    until: git2::Time,
) -> Result<Vec<GlobalCommit>, GglError> {
    let mut commits: Vec<GlobalCommit> = vec![];
    for r in &config.repositories {
        let repo_path = Path::new(&config.root).join(&r.path);
        let repo = git2::Repository::open(repo_path)?;

        if fetch {
            git_fetch(&repo, r)?;
        }

        let res = collect_commits_for_repo(repo, &r, until);
        match res {
            Ok(c) => {
                commits.extend(c);
            }
            Err(e) => {
                println!("{:?}", e);
            }
        }
    }
    commits.sort_by_key(|commit| commit.date);
    commits.reverse();
    Ok(commits)
}

fn collect_commits_for_repo(
    repo: git2::Repository,
    r: &Repository,
    until: git2::Time,
) -> Result<Vec<GlobalCommit>, GglError> {
    let mut commits: Vec<GlobalCommit> = vec![];
    let mut revwalk = repo.revwalk()?;
    revwalk.push_head()?;
    let mut diffopts = git2::DiffOptions::new();

    for id in revwalk {
        let id = id?;
        let commit = repo.find_commit(id)?;
        let commit_date = commit.author().when();

        if commit_date < until {
            break;
        }

        if let Some(filters) = &r.filters {
            let mut changed_files: Vec<PathBuf> = vec![];
            let a = if commit.parents().len() == 1 {
                let parent = commit.parent(0)?;
                Some(parent.tree()?)
            } else {
                None
            };
            let b = commit.tree()?;
            let diff = repo.diff_tree_to_tree(a.as_ref(), Some(&b), Some(&mut diffopts))?;

            for delta in diff.deltas() {
                let new_file = delta.new_file();
                changed_files.push(new_file.path().unwrap().to_owned());
            }

            if !should_be_included(&filters, &changed_files) {
                continue;
            }
        }

        let global_commit = GlobalCommit {
            author: commit.author().name().unwrap().to_string(),
            date: commit.author().when(),
            message: commit.message().unwrap().to_string(),
            sha: commit.id().to_string(),
            repo_name: r.name.clone(),
        };

        commits.push(global_commit);
    }

    Ok(commits)
}

fn print_global_commit(commit: &GlobalCommit) {
    let commit_line = format!("commit {}", commit.sha);
    println!("{}", commit_line.yellow());
    println!("Repo:   {}", commit.repo_name);
    println!("Author: {}", commit.author);
    print_time(&commit.date, "Date:   ");
    println!();

    for line in commit.message.lines() {
        println!("    {}", line);
    }

    println!();
}

fn print_time(time: &git2::Time, prefix: &str) {
    let (offset, sign) = match time.offset_minutes() {
        n if n < 0 => (-n, '-'),
        n => (n, '+'),
    };
    let (hours, minutes) = (offset / 60, offset % 60);
    let ts = time::Timespec::new(time.seconds() + (time.offset_minutes() as i64) * 60, 0);
    let time = time::at(ts);

    println!(
        "{}{} {}{:02}{:02}",
        prefix,
        time.strftime("%a %b %e %T %Y").unwrap(),
        sign,
        hours,
        minutes
    );
}

fn get_until(arg: &Option<String>) -> i64 {
    let date = match arg {
        Some(date) => chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d").unwrap(),
        None => chrono::Local::now()
            .date_naive()
            .sub(chrono::Duration::weeks(1)),
    };

    date.and_hms_opt(0, 0, 0).unwrap().timestamp()
}

// Look for a config file in the following places in the following order:
//   1.  --config flag
//   2.  $XDG_CONFIG_HOME/ggl.yaml
//   3.  config.yaml in the current directory
fn get_config_path(arg_config: Option<PathBuf>) -> Result<PathBuf, GglError> {
    let path = match arg_config {
        Some(pb) => pb,
        None => match dirs::config_dir() {
            Some(pb) => pb.join("ggl.yaml").to_path_buf(),
            None => PathBuf::from("config.yaml"),
        },
    };

    if !path.exists() {
        return Err(GglError::MissingConfigFile);
    }

    Ok(path)
}

fn run(args: &Args) -> Result<(), GglError> {
    let config_path = get_config_path(args.config.clone())?;
    let config = load_config(config_path)?;
    let until = git2::Time::new(get_until(&args.until), 0);
    let commits = collect_commits(&config, args.fetch, until)?;

    for commit in commits {
        print_global_commit(&commit);
    }

    Ok(())
}

fn main() {
    let args = Args::from_args();
    match run(&args) {
        Ok(()) => {}
        Err(e) => println!("error: {:?}", e),
    }
}
