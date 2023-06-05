use octocrab::{models::pulls::PullRequest, Octocrab};
use serde::{Deserialize, Serialize};

#[derive(Serialize)]
struct GenerateReleaseNoteInput {
    tag_name: String,
    target_commitish: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    previous_tag_name: Option<String>,
}

#[derive(Deserialize)]
struct GenerateReleaseNoteOutput {
    body: String,
}

pub struct Repo {
    client: Octocrab,
    owner: String,
    name: String,
}

impl Repo {
    pub fn new(client: Octocrab, owner: String, name: String) -> Repo {
        Repo {
            client,
            owner,
            name,
        }
    }

    async fn generate_release_note(
        &self,
        tag: &str,
        latest_tag: Option<String>,
        target_commit: &str,
    ) -> Result<String, octocrab::Error> {
        let out = self
            .client
            .post::<GenerateReleaseNoteInput, GenerateReleaseNoteOutput>(
                format!(
                    "/repos/{}/{}/releases/generate-notes",
                    self.owner, self.name
                ),
                Some(&GenerateReleaseNoteInput {
                    tag_name: tag.to_string(),
                    target_commitish: target_commit.to_string(),
                    previous_tag_name: latest_tag,
                }),
            )
            .await?;
        Ok(out.body)
    }

    pub async fn create_or_update_pr(
        &self,
        branch: &str,
        base_branch: &str,
        latest_tag: Option<String>,
        title: &str,
    ) -> Result<String, String> {
        let note = self
            .generate_release_note("next-release", latest_tag, branch)
            .await
            .map_err(|e| format!("Failed to generate PR body: {}", e))?;

        let mut pages = self
            .client
            .pulls(&self.owner, &self.name)
            .list()
            .base(base_branch)
            .head(format!("{}:{}", &self.owner, branch))
            .per_page(1)
            .page(1u32)
            .send()
            .await
            .map_err(|e| format!("Failed to get the current release PR: {}", e))?;

        match pages.take_items().get(0) {
            Some(pr) => {
                self.update_pr(pr.number, &note)
                    .await
                    .map_err(|e| format!("Failed to update the release PR: {}", e))?;
                let url = pr.html_url.clone().unwrap();
                Ok(url.to_string())
            }
            None => {
                let url = self
                    .create_pr(branch, base_branch, title, &note)
                    .await
                    .map_err(|e| format!("Failed to create the new release PR: {}", e))?;
                Ok(url)
            }
        }
    }

    async fn create_pr(
        &self,
        branch: &str,
        base_branch: &str,
        title: &str,
        body: &str,
    ) -> Result<String, octocrab::Error> {
        let pr = self
            .client
            .pulls(&self.owner, &self.name)
            .create(title, branch, base_branch)
            .body(body)
            .send()
            .await?;
        Ok(pr.html_url.unwrap().to_string())
    }

    async fn update_pr(&self, pr_id: u64, body: &str) -> Result<(), octocrab::Error> {
        self.client
            .pulls(&self.owner, &self.name)
            .update(pr_id)
            .body(body)
            .send()
            .await?;
        Ok(())
    }

    pub async fn is_release_commit(
        &self,
        commit: &str,
        release_branch: &str,
    ) -> Result<bool, String> {
        let pr = self
            .related_pr(commit)
            .await
            .map_err(|e| format!("Failed to check commit: {}", e))?;
        match pr {
            Some(pr) => Ok(pr.head.ref_field == release_branch),
            None => Ok(false),
        }
    }

    async fn related_pr(&self, commit: &str) -> Result<Option<PullRequest>, octocrab::Error> {
        let q = format!(
            "repo:{}/{} type:pr is:merged {}",
            self.owner, self.name, commit
        );

        let mut pages = self
            .client
            .search()
            .issues_and_pull_requests(q.as_str())
            .sort("best match")
            .order("asc")
            .send()
            .await?;

        let pr_id = match pages.take_items().get(0) {
            Some(pr) => pr.number,
            None => return Ok(None),
        };

        let pr = self
            .client
            .pulls(&self.owner, &self.name)
            .get(pr_id)
            .await?;

        Ok(Some(pr))
    }
}
