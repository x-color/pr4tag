mod git;
mod github;

use std::{fs::OpenOptions, io::Write};

use chrono::Local;
use git2::Repository;
use octocrab::Octocrab;
use tokio;

#[tokio::main]
async fn main() {
    let token = std::env::var("GITHUB_TOKEN").expect("GITHUB_TOKEN env variable is required");
    let release_branch = std::env::var("RELEASE_BRANCH").unwrap_or("release/next".to_string());
    let tag_prefix = std::env::var("TAG_PREFIX").unwrap_or("release".to_string());
    let pr_title = std::env::var("PR_TITLE").unwrap_or("Release".to_string());

    let repo = git::open_repo(".").unwrap();

    let remote_repo = git::remote_repo(&repo).unwrap();
    let (owner, rp) = remote_repo.split_once('/').unwrap();
    let octocrab = Octocrab::builder().personal_token(token).build().unwrap();
    let gh_repo = github::Repo::new(octocrab, owner.to_string(), rp.to_string());

    let commit = git::latest_commit_id(&repo).unwrap();
    let released = gh_repo
        .is_release_commit(&commit, &release_branch)
        .await
        .unwrap();

    if released {
        println!("The latest commit is for release");
        if already_tagged(&repo, &tag_prefix).unwrap() {
            println!("The latest commit is already released");
            return;
        }
        let tag = create_release_tag(&repo, &tag_prefix).unwrap();
        gh_output("tag", &tag).unwrap();
        return;
    }

    println!("The latest commit is not for release");
    let pr_id = create_release_pr(&repo, &gh_repo, &release_branch, &tag_prefix, &pr_title)
        .await
        .unwrap();
    gh_output("pr_id", &pr_id).unwrap();
}

fn already_tagged(repo: &Repository, prefix: &str) -> Result<bool, String> {
    let tag_name = git::latest_tag(&repo, &format!("{}-*", prefix))?;
    if tag_name == None {
        return Ok(false);
    }
    let tag_name = tag_name.unwrap();
    let tid = git::tag_commit_id(&repo, &tag_name)?;
    let cid = git::latest_commit_id(&repo)?;
    if tid == cid {
        println!("The latest commit has {}", tag_name);
        return Ok(true);
    }
    Ok(false)
}

fn create_release_tag(repo: &Repository, prefix: &str) -> Result<String, String> {
    let dt = Local::now();
    let tag = format!("{}-{}", prefix, dt.format("%Y%m%d%H%M%S"));
    git::tag_for_release(&repo, &tag)?;
    git::push_release_tag(&repo, &tag)?;

    println!("Release Tag: {}", tag);
    Ok(tag)
}

async fn create_release_pr(
    repo: &Repository,
    gh_repo: &github::Repo,
    release_branch: &str,
    tag_prefix: &str,
    pr_title: &str,
) -> Result<String, String> {
    let base_branch = git::base_branch(&repo)?;
    println!("Base branch: {}", base_branch);

    let full_branch_name = git::move_to_release_branch(&repo, &release_branch)?;
    println!("Create release branch: {}", full_branch_name);

    let commit_id = git::commit_for_release(&repo)?;
    println!("Create release commit: {}", commit_id);

    git::push_release_branch(&repo, &full_branch_name)?;
    println!(
        "Push release branch: {}",
        full_branch_name.replace("refs/heads/", "remotes/origin/")
    );

    let latest_tag = git::latest_tag(&repo, &format!("{}-*", tag_prefix))?;
    let pr_id = gh_repo
        .create_or_update_pr(
            &full_branch_name.trim_start_matches("refs/heads/"),
            &base_branch,
            latest_tag,
            &pr_title,
        )
        .await?;
    println!("Release PR: {}", pr_id);
    Ok(pr_id)
}

fn gh_output(name: &str, value: &str) -> Result<(), String> {
    let file_path = std::env::var("GITHUB_OUTPUT");
    let file_path = match file_path {
        Ok(f) => f,
        Err(_) => return Ok(()),
    };

    // Open the file in append mode
    let mut file = OpenOptions::new()
        .write(true)
        .append(true)
        .open(&file_path)
        .map_err(|e| format!("Failed to open {}: {}", &file_path, e))?;

    // Write lines to the file
    file.write(format!("{}={}\n", name, value).as_bytes())
        .map_err(|e| format!("Failed to open {}: {}", &file_path, e))?;

    Ok(())
}
