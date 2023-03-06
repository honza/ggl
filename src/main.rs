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

use colored::*;
use dirs;
use git2;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::str;
use structopt::StructOpt;
use time;

// git format: Wed Nov 16 11:05:18 2022 -0400
static DATETIME: &str = "[weekday repr:short] [month repr:short] \
                         [day padding:none] [hour]:[minute]:[second] \
                         [year] [offset_hour sign:mandatory][offset_minute]";

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

    #[structopt(name = "reverse", long, short)]
    /// Reverse the result
    reverse: bool,

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
struct Block {
    root: String,
    repositories: Vec<Repository>,
}

#[derive(Debug, Deserialize)]
struct Config {
    blocks: Vec<Block>,
}

#[derive(Debug, Serialize, Clone)]
struct GlobalCommit {
    author: String,
    date: time::OffsetDateTime,
    message: String,
    repo_name: String,
    sha: String,
}

/// A CommitSet represents a unit of change to a repo.  It's either:
///
/// 1.  A single commit committed to your selected branch
/// 2.  One or more commits introduced to your branch by a merge commit
///
/// The purpose of this tool is to find commits that broke things.
///
/// Here is how we create these sets:
///
/// For every repository, we walk in topological order.
///
/// If we see a commit that isn't a merge, we create a set with a single item.
/// The date is the date of that commit.
///
/// If we see a commit that is a merge, we collect commits until we hit the
/// SHA of the first parent of that commit.  The date is the date of the
/// merge commit.
///
/// These CommitSets can then be sorted by date, and printed.
#[derive(Debug)]
struct CommitSet {
    date: time::OffsetDateTime,
    commits: Vec<GlobalCommit>,
}

type CommitSetResult = Result<Vec<CommitSet>, GglError>;

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
    if filters.len() == 0 {
        return true;
    }
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

        // If we didn't find a match above
        match filter.filter_type {
            FilterType::Include => {
                return false;
            }
            FilterType::Reject => {
                return true;
            }
        }
    }

    // This should never happen :)
    true
}

fn collect_commitsets(config: &Config, fetch: bool, until: git2::Time) -> CommitSetResult {
    let mut commitsets: Vec<CommitSet> = vec![];
    for block in &config.blocks {
        for r in &block.repositories {
            let repo_path = Path::new(&block.root).join(&r.path);
            let repo = git2::Repository::open(repo_path)?;

            if fetch {
                git_fetch(&repo, r)?;
            }

            let sets = collect_commitsets_for_repo(repo, &r, until)?;
            commitsets.extend(sets);
        }
    }
    commitsets.sort_by_key(|set| set.date);
    commitsets.reverse();
    Ok(commitsets)
}

fn collect_commitsets_for_repo(
    repo: git2::Repository,
    r: &Repository,
    until: git2::Time,
) -> CommitSetResult {
    let mut commitsets: Vec<CommitSet> = vec![];
    let mut revwalk = repo.revwalk()?;
    let git_ref = format!("refs/remotes/{}/{}", r.remote, r.branch);
    revwalk.push_ref(&git_ref)?;
    revwalk.set_sorting(git2::Sort::TOPOLOGICAL)?;
    let mut diffopts = git2::DiffOptions::new();

    let mut commit_buffer: Vec<GlobalCommit> = vec![];
    let mut collecting_commits = false;
    let mut set_date: time::OffsetDateTime = time::OffsetDateTime::now_utc();
    let mut destination_commit_id: git2::Oid = git2::Oid::zero();

    for id in revwalk {
        let id = id?;
        let commit = repo.find_commit(id)?;
        let commit_date = commit.author().when();

        if commit_date < until {
            break;
        }

        let is_merge = commit.parent_count() > 1;

        if !is_merge {
            if let Some(filters) = &r.filters {
                let mut changed_files: Vec<PathBuf> = vec![];
                let current_tree = commit.tree()?;

                let parent_tree = if commit.parent_count() == 1 {
                    Some(commit.parent(0)?.tree()?)
                } else {
                    None
                };

                let diff = repo.diff_tree_to_tree(
                    parent_tree.as_ref(),
                    Some(&current_tree),
                    Some(&mut diffopts),
                )?;

                for delta in diff.deltas() {
                    let new_file = delta.new_file();
                    changed_files.push(new_file.path().unwrap().to_owned());
                }

                if !should_be_included(filters, &changed_files) {
                    continue;
                }
            }
        }

        if collecting_commits && commit.id() == destination_commit_id {
            let set = CommitSet {
                date: set_date,
                commits: commit_buffer.clone(),
            };

            // reset
            commit_buffer.clear();
            collecting_commits = false;
            commitsets.push(set);
        }

        let commit_date = git_time_to_datetime(&commit.author().when())?;

        let global_commit = GlobalCommit {
            author: commit.author().name().unwrap().to_string(),
            date: commit_date.clone(),
            message: commit.message().unwrap().to_string(),
            sha: commit.id().to_string(),
            repo_name: r.name.clone(),
        };

        if is_merge {
            set_date = commit_date.clone();
            collecting_commits = true;
            destination_commit_id = commit.parent(0)?.id();

            commit_buffer.push(global_commit);
        } else {
            if collecting_commits {
                commit_buffer.push(global_commit);
                continue;
            }

            let set = CommitSet {
                date: commit_date,
                commits: vec![global_commit],
            };

            commitsets.push(set);
        }
    }

    Ok(commitsets)
}

fn print_commit_set(set: &mut CommitSet, reverse: bool) {
    if reverse {
        set.commits.reverse();
    }

    for commit in &set.commits {
        print_global_commit(commit);
    }
}

fn print_global_commit(commit: &GlobalCommit) {
    let commit_line = format!("commit {}", commit.sha);
    println!("{}", commit_line.yellow());
    println!("Repo:   {}", commit.repo_name);
    println!("Author: {}", commit.author);
    print_time(&commit.date);
    println!();

    for line in commit.message.lines() {
        println!("    {}", line);
    }

    println!();
}

fn git_time_to_datetime(time: &git2::Time) -> Result<time::OffsetDateTime, GglError> {
    let off = time::UtcOffset::from_whole_seconds(time.offset_minutes() * 60).unwrap();

    let ts = time::OffsetDateTime::from_unix_timestamp(
        time.seconds() + (time.offset_minutes() as i64) * 60,
    )
    .unwrap()
    .replace_offset(off);

    Ok(ts)
}

fn print_time(t: &time::OffsetDateTime) {
    // Not sure how to do a global const that reqires a function call
    let f = time::format_description::parse(DATETIME).unwrap();
    let s = t.format(&f).unwrap();
    println!("Date:   {}", s);
}

fn get_until(arg: &Option<String>) -> i64 {
    match arg {
        Some(date) => {
            let format = time::macros::format_description!("[year]-[month]-[day]");
            let offset = time::UtcOffset::current_local_offset().unwrap();
            time::Date::parse(date, &format)
                .unwrap()
                .with_hms(0, 0, 0)
                .unwrap()
                .assume_offset(offset)
                .unix_timestamp()
        }
        None => time::OffsetDateTime::now_local()
            .unwrap()
            .saturating_sub(time::Duration::days(7))
            .unix_timestamp(),
    }
}

// Look for a config file in the following places in the following order:
//   1.  --config flag
//   2.  $XDG_CONFIG_HOME/ggl.yaml
//   3.  config.yaml in the current directory
fn get_config_path(arg_config: Option<PathBuf>) -> Result<PathBuf, GglError> {
    if let Some(path) = arg_config {
        if path.exists() {
            return Ok(path);
        } else {
            return Err(GglError::MissingConfigFile);
        }
    }

    if let Some(path) = dirs::config_dir() {
        let full_path = path.join("ggl.yaml").to_path_buf();
        if full_path.exists() {
            return Ok(full_path);
        }
    }

    let local_file = PathBuf::from("config.yaml");
    if local_file.exists() {
        return Ok(local_file);
    }

    return Err(GglError::MissingConfigFile);
}

fn print_json(sets: &mut Vec<CommitSet>, reverse: bool) {
    let mut commits: Vec<&GlobalCommit> = vec![];

    for set in sets {
        if reverse {
            set.commits.reverse();
        }
        for commit in &set.commits {
            commits.push(commit);
        }
    }

    match serde_json::to_string(&commits) {
        Ok(c) => println!("{}", c),
        Err(e) => println!("Errror {:?}", e),
    }
}

fn run(args: &Args) -> Result<(), GglError> {
    let config_path = get_config_path(args.config.clone())?;
    let config = load_config(config_path)?;
    let until = git2::Time::new(get_until(&args.until), 0);
    let mut commitsets = collect_commitsets(&config, args.fetch, until)?;

    if args.reverse {
        commitsets.reverse();
    }

    if args.json {
        print_json(&mut commitsets, args.reverse);
    } else {
        for set in commitsets.iter_mut() {
            print_commit_set(set, args.reverse);
        }
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
