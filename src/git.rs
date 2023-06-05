use git2::Repository;

pub fn open_repo(path: &str) -> Result<Repository, String> {
    Repository::open(path).map_err(|e| format!("Failed to open repository: {}", e))
}

pub fn base_branch(repo: &Repository) -> Result<String, String> {
    let head = repo
        .head()
        .map_err(|e| format!("Failed to get base branch: {}", e))?;
    let name = head
        .name()
        .ok_or("Failed to get base branch name".to_string())?;
    Ok(name.to_string())
}

pub fn remote_repo(repo: &Repository) -> Result<String, String> {
    let remote = repo
        .find_remote("origin")
        .map_err(|e| format!("Failed to get base branch: {}", e))?;
    let n = remote
        .url()
        .ok_or("Remote repository url is not found".to_string())?;
    let l: Vec<&str> = n.rsplitn(3, '/').collect();
    if l.len() < 2 {
        return Err("Unexpected remote repository url".to_string());
    }
    Ok(format!("{}/{}", l[1], l[0].trim_end_matches(".git")))
}

pub fn move_to_release_branch(repo: &Repository, branch_name: &str) -> Result<String, String> {
    let head_commit = repo
        .head()
        .map_err(|e| format!("Failed to get HEAD: {}", e))?
        .peel_to_commit()
        .map_err(|e| format!("Failed to get HEAD commit: {}", e))?;
    let branch = repo
        .branch(&branch_name, &head_commit, true)
        .map_err(|e| format!("Failed to create branch: {}", e))?;
    let full_branch_name = branch.get().name().ok_or("Failed to get branch name")?;

    let obj = repo
        .revparse_single(full_branch_name)
        .map_err(|e| format!("Failed to get object: {}", e))?;
    repo.checkout_tree(&obj, None)
        .map_err(|e| format!("Failed to checkout: {}", e))?;
    repo.set_head(full_branch_name)
        .map_err(|e| format!("Failed to move HEAD: {}", e))?;
    Ok(full_branch_name.to_string())
}

pub fn commit_for_release(repo: &Repository) -> Result<String, String> {
    let commit_id =
        commit(repo, "Release commit").map_err(|e| format!("Failed to commit: {}", e))?;
    Ok(commit_id)
}

fn commit(repo: &Repository, msg: &str) -> Result<String, git2::Error> {
    let oid = repo.index()?.write_tree()?;
    let tree = repo.find_tree(oid)?;
    let head_commit = repo.head()?.peel_to_commit()?;
    let sig = repo.signature()?;
    let commit_id = repo.commit(Some("HEAD"), &sig, &sig, msg, &tree, &[&head_commit])?;
    Ok(commit_id.to_string())
}

pub fn latest_tag(repo: &Repository, pattern: &str) -> Result<Option<String>, String> {
    let tags = repo
        .tag_names(Some(pattern))
        .map_err(|e| format!("Failed to get tags: {}", e))?;
    match tags.iter().last() {
        Some(tag) => match tag {
            Some(tag) => Ok(Some(tag.to_string())),
            None => Ok(None),
        },
        None => Ok(None),
    }
}

pub fn tag_commit_id(repo: &Repository, tag_name: &str) -> Result<String, String> {
    let obj = repo
        .revparse_single(&format!("refs/tags/{}", tag_name))
        .map_err(|e| format!("Failed to find tag: {}", e))?;
    let tag = obj
        .into_tag()
        .map_err(|_| "Failed to get tag info".to_string())?;
    Ok(tag.target_id().to_string())
}

pub fn latest_commit_id(repo: &Repository) -> Result<String, String> {
    let commit = latest_commit(repo).map_err(|e| format!("Failed to find latest commit: {}", e))?;
    Ok(commit.id().to_string())
}

fn latest_commit(repo: &Repository) -> Result<git2::Commit, String> {
    let commit = repo.head().map_err(|e| e.to_string())?;
    let oid = commit.target().ok_or("Can not get object id of commit")?;
    repo.find_commit(oid).map_err(|e| e.to_string())
}

pub fn tag_for_release(repo: &Repository, tag: &str) -> Result<(), String> {
    let sig = repo
        .signature()
        .map_err(|e| format!("Failed to get signature for git: {}", e))?;
    let commit = latest_commit(repo).map_err(|e| format!("Failed to find latest commit: {}", e))?;
    repo.tag(&tag, &commit.into_object(), &sig, "", false)
        .map_err(|e| format!("Failed to create tag: {}", e))?;
    Ok(())
}

pub fn push_release_branch(repo: &Repository, branch: &str) -> Result<(), String> {
    let refspec = format!("+{}:{}", branch, branch);
    push(repo, &refspec).map_err(|e| format!("Failed to push branch: {}", e))
}

pub fn push_release_tag(repo: &Repository, tag: &str) -> Result<(), String> {
    let refspec = format!("refs/tags/{}", tag);
    push(repo, &refspec).map_err(|e| format!("Failed to push tag: {}", e))
}

fn push(repo: &Repository, refspec: &str) -> Result<(), git2::Error> {
    let mut remote = repo.find_remote("origin")?;
    remote.push(&[refspec], None)
}
