use git2::Error;
use git2::{Commit, DiffOptions, Time};
use serde::Deserialize;
use std::fs;
use std::path::Path;
use std::str;
use structopt::StructOpt;

#[derive(StructOpt)]
struct Args {
    #[structopt(name = "dir", long = "git-dir")]
    /// alternative git directory to use
    flag_git_dir: Option<String>,

    #[structopt(name = "max-count", short = "n", long)]
    /// maximum number of commits to show
    flag_max_count: Option<usize>,

    #[structopt(name = "patch", long, short)]
    /// show commit diff
    flag_patch: bool,
}

#[derive(Debug, Deserialize)]
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
struct GlobalCommit<'a> {
    commit: Commit<'a>,
    repo_name: String,
    changed_files: Vec<String>,
}

fn load_config() -> Result<Config, serde_yaml::Error> {
    let contents = fs::read_to_string("config.yaml").unwrap();
    serde_yaml::from_str(&contents)
}

fn collect_commits(config: &Config, until: Time) -> Result<Vec<GlobalCommit>, Error> {
    let commits: Vec<GlobalCommit> = vec![];
    for r in &config.repositories {
        let repo_path = Path::new(&config.root).join(&r.path);
        let repo = git2::Repository::open(repo_path)?;
        let c = collect_commits_for_repo(&repo, r.name.clone(), until);
        println!("{:?}", c);
        // TODO: sort by date
    }
    Ok(commits)
}

fn collect_commits_for_repo(
    repo: &git2::Repository,
    repo_name: String,
    until: Time,
) -> Result<Vec<GlobalCommit>, Error> {
    let mut commits: Vec<GlobalCommit> = vec![];
    let mut revwalk = repo.revwalk()?;
    revwalk.push_head()?;
    let mut diffopts = DiffOptions::new();

    for id in revwalk {
        let id = id?;
        let commit = repo.find_commit(id)?;

        let commit_date = commit.author().when();
        println!("{:?}", until);
        print_time(&until, "");
        print_time(&commit_date, "");

        if commit_date < until {
            break;
        }

        let a = if commit.parents().len() == 1 {
            let parent = commit.parent(0)?;
            Some(parent.tree()?)
        } else {
            None
        };
        let b = commit.tree()?;
        let diff = repo.diff_tree_to_tree(a.as_ref(), Some(&b), Some(&mut diffopts))?;

        let mut changed_files: Vec<String> = vec![];

        for delta in diff.deltas() {
            let new_file = delta.new_file();
            changed_files.push(new_file.path().unwrap().to_str().unwrap().to_owned());
        }

        let global_commit = GlobalCommit {
            commit: commit,
            repo_name: repo_name.clone(),
            changed_files: changed_files,
        };

        commits.push(global_commit);
    }

    Ok(commits)
}

fn run(_args: &Args) -> Result<(), Error> {
    let config = load_config().unwrap();
    println!("{:?}", config);

    let until = Time::new(1667500000, 0);
    collect_commits(&config, until)?;

    Ok(())
}

fn print_commit(commit: &Commit) {
    println!("commit {}", commit.id());

    if commit.parents().len() > 1 {
        print!("Merge:");
        for id in commit.parent_ids() {
            print!(" {:.8}", id);
        }
        println!();
    }

    let author = commit.author();
    println!("Author: {}", author);
    print_time(&author.when(), "Date:   ");
    println!();

    for line in String::from_utf8_lossy(commit.message_bytes()).lines() {
        println!("    {}", line);
    }
    println!();
}

fn print_time(time: &Time, prefix: &str) {
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

fn main() {
    let args = Args::from_args();
    match run(&args) {
        Ok(()) => {}
        Err(e) => println!("error: {}", e),
    }
}
