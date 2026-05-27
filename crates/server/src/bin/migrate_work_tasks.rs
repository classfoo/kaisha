use std::{env, path::PathBuf};

fn main() -> anyhow::Result<()> {
    let workspace = env::var("KAISHA_WORKDIR").map(PathBuf::from).or_else(|_| {
        env::var("HOME").map(|home| PathBuf::from(home).join(".kaisha"))
    })?;

    let report = server::work_task_reconcile::reconcile_workspace_work_tasks(&workspace);
    println!(
        "reconciled workspace {} (requirements={}, development={}, review={}, errors={})",
        workspace.display(),
        report.requirements_processed,
        report.development_reconciled,
        report.review_reconciled,
        report.errors.len()
    );
    for err in &report.errors {
        eprintln!("error: {err}");
    }
    if report.errors.is_empty() {
        Ok(())
    } else {
        anyhow::bail!("work task reconcile finished with errors")
    }
}
