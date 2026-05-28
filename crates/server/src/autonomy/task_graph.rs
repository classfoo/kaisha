use crate::autonomy::task::{ComplexityLevel, Task, TaskPriority, TaskStatus};
use std::{
    collections::{BinaryHeap, HashMap, HashSet},
    cmp::Ordering,
    path::Path,
};

#[derive(Debug, Clone)]
pub struct ReadyTask {
    pub task_id: String,
    pub priority: TaskPriority,
    pub created_at_ms: u64,
    pub complexity: ComplexityLevel,
}

impl PartialEq for ReadyTask {
    fn eq(&self, other: &Self) -> bool {
        self.task_id == other.task_id
    }
}

impl Eq for ReadyTask {}

impl PartialOrd for ReadyTask {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ReadyTask {
    fn cmp(&self, other: &Self) -> Ordering {
        self.priority.cmp(&other.priority)
            .then_with(|| other.created_at_ms.cmp(&self.created_at_ms))
            .then_with(|| other.complexity.cmp(&self.complexity))
    }
}

#[derive(Debug, Clone)]
pub struct TaskNode {
    pub task: Task,
    pub remaining_deps: usize,
}

#[derive(Debug)]
pub struct TaskGraph {
    tasks: HashMap<String, TaskNode>,
    ready_queue: BinaryHeap<ReadyTask>,
    dependents: HashMap<String, HashSet<String>>,
}

impl TaskGraph {
    pub fn new() -> Self {
        Self {
            tasks: HashMap::new(),
            ready_queue: BinaryHeap::new(),
            dependents: HashMap::new(),
        }
    }

    pub fn add_task(&mut self, task: Task) -> anyhow::Result<()> {
        if self.tasks.contains_key(&task.id) {
            anyhow::bail!("task_already_exists: {}", task.id);
        }

        let remaining_deps = task.dependencies.len();
        let node = TaskNode {
            task: task.clone(),
            remaining_deps,
        };
        self.tasks.insert(task.id.clone(), node);

        for dep_id in &task.dependencies {
            self.dependents
                .entry(dep_id.clone())
                .or_default()
                .insert(task.id.clone());
        }

        if task.dependencies.is_empty() && task.status == TaskStatus::Pending {
            self.mark_ready(&task.id)?;
        }

        Ok(())
    }

    pub fn mark_ready(&mut self, task_id: &str) -> anyhow::Result<()> {
        let node = self.tasks.get_mut(task_id)
            .ok_or_else(|| anyhow::anyhow!("task_not_found: {}", task_id))?;

        if node.task.status != TaskStatus::Ready && node.remaining_deps == 0 {
            node.task.status = TaskStatus::Ready;
            let task = &node.task;
            self.ready_queue.push(ReadyTask {
                task_id: task.id.clone(),
                priority: task.priority,
                created_at_ms: task.created_at_ms,
                complexity: task.complexity,
            });
        }

        Ok(())
    }

    pub fn complete_task(&mut self, task_id: &str) -> anyhow::Result<Vec<String>> {
        let node = self.tasks.get_mut(task_id)
            .ok_or_else(|| anyhow::anyhow!("task_not_found: {}", task_id))?;
        node.task.status = TaskStatus::Completed;

        let dependents = self.dependents.get(task_id).cloned().unwrap_or_default();
        let mut newly_ready = Vec::new();

        for dep_id in dependents {
            if let Some(dep_node) = self.tasks.get_mut(&dep_id) {
                dep_node.remaining_deps = dep_node.remaining_deps.saturating_sub(1);
                if dep_node.remaining_deps == 0 && dep_node.task.status == TaskStatus::Pending {
                    dep_node.task.status = TaskStatus::Ready;
                    let task = &dep_node.task;
                    self.ready_queue.push(ReadyTask {
                        task_id: task.id.clone(),
                        priority: task.priority,
                        created_at_ms: task.created_at_ms,
                        complexity: task.complexity,
                    });
                    newly_ready.push(dep_id);
                }
            }
        }

        Ok(newly_ready)
    }

    pub fn fail_task(&mut self, task_id: &str, _error: &str) -> anyhow::Result<()> {
        let node = self.tasks.get_mut(task_id)
            .ok_or_else(|| anyhow::anyhow!("task_not_found: {}", task_id))?;
        node.task.status = TaskStatus::Failed;
        Ok(())
    }

    pub fn cancel_task(&mut self, task_id: &str) -> anyhow::Result<()> {
        let node = self.tasks.get_mut(task_id)
            .ok_or_else(|| anyhow::anyhow!("task_not_found: {}", task_id))?;
        node.task.status = TaskStatus::Cancelled;
        Ok(())
    }

    pub fn schedule_task(&mut self, task_id: &str, assignee: &str) -> anyhow::Result<()> {
        let node = self.tasks.get_mut(task_id)
            .ok_or_else(|| anyhow::anyhow!("task_not_found: {}", task_id))?;
        node.task.status = TaskStatus::Scheduled;
        node.task.assignee = Some(assignee.to_string());
        Ok(())
    }

    pub fn start_task(&mut self, task_id: &str) -> anyhow::Result<()> {
        let node = self.tasks.get_mut(task_id)
            .ok_or_else(|| anyhow::anyhow!("task_not_found: {}", task_id))?;
        node.task.status = TaskStatus::Running;
        Ok(())
    }

    pub fn pause_task(&mut self, task_id: &str) -> anyhow::Result<()> {
        let node = self.tasks.get_mut(task_id)
            .ok_or_else(|| anyhow::anyhow!("task_not_found: {}", task_id))?;
        node.task.status = TaskStatus::Paused;
        Ok(())
    }

    pub fn pop_ready(&mut self) -> Option<ReadyTask> {
        self.ready_queue.pop()
    }

    pub fn push_ready(&mut self, ready: ReadyTask) {
        self.ready_queue.push(ready);
    }

    pub fn get_task(&self, task_id: &str) -> Option<&Task> {
        self.tasks.get(task_id).map(|n| &n.task)
    }

    pub fn get_task_mut(&mut self, task_id: &str) -> Option<&mut Task> {
        self.tasks.get_mut(task_id).map(|n| &mut n.task)
    }

    pub fn ready_queue_len(&self) -> usize {
        self.ready_queue.len()
    }

    pub fn all_tasks(&self) -> &HashMap<String, TaskNode> {
        &self.tasks
    }

    pub fn has_circular_dependency(&self) -> Option<Vec<String>> {
        let mut visited = HashSet::new();
        let mut rec_stack = HashSet::new();
        let mut cycle = Vec::new();

        for task_id in self.tasks.keys() {
            if !visited.contains(task_id) {
                if self.dfs_cycle(task_id, &mut visited, &mut rec_stack, &mut cycle) {
                    return Some(cycle);
                }
            }
        }
        None
    }

    fn dfs_cycle(
        &self,
        task_id: &str,
        visited: &mut HashSet<String>,
        rec_stack: &mut HashSet<String>,
        cycle: &mut Vec<String>,
    ) -> bool {
        visited.insert(task_id.to_string());
        rec_stack.insert(task_id.to_string());

        if let Some(node) = self.tasks.get(task_id) {
            for dep_id in &node.task.dependencies {
                if !visited.contains(dep_id) {
                    if self.dfs_cycle(dep_id, visited, rec_stack, cycle) {
                        cycle.push(task_id.to_string());
                        return true;
                    }
                } else if rec_stack.contains(dep_id) {
                    cycle.push(dep_id.to_string());
                    cycle.push(task_id.to_string());
                    return true;
                }
            }
        }

        rec_stack.remove(task_id);
        false
    }

    pub fn persist(&self, workspace: &Path) -> anyhow::Result<()> {
        let graph_data = TaskGraphData {
            tasks: self.tasks.iter().map(|(k, v)| (k.clone(), v.task.clone())).collect(),
            ready_queue: self.ready_queue.iter().map(|r| r.task_id.clone()).collect(),
            dependents: self.dependents.iter().map(|(k, v)| (k.clone(), v.iter().cloned().collect())).collect(),
        };
        let path = workspace.join("autonomy").join("task_graph").join("graph.json");
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, serde_json::to_string_pretty(&graph_data)?)?;
        Ok(())
    }

    pub fn load(workspace: &Path) -> anyhow::Result<Self> {
        let path = workspace.join("autonomy").join("task_graph").join("graph.json");
        if !path.exists() {
            return Ok(Self::new());
        }
        let raw = std::fs::read_to_string(&path)?;
        let data: TaskGraphData = serde_json::from_str(&raw)?;

        let mut graph = Self::new();
        for (id, task) in data.tasks {
            let remaining_deps = task.dependencies.len();
            graph.tasks.insert(id, TaskNode { task, remaining_deps });
        }
        graph.dependents = data.dependents;

        for (task_id, node) in &graph.tasks {
            if node.remaining_deps == 0 && node.task.status == TaskStatus::Ready {
                graph.ready_queue.push(ReadyTask {
                    task_id: task_id.clone(),
                    priority: node.task.priority,
                    created_at_ms: node.task.created_at_ms,
                    complexity: node.task.complexity,
                });
            }
        }

        Ok(graph)
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct TaskGraphData {
    tasks: HashMap<String, Task>,
    ready_queue: Vec<String>,
    dependents: HashMap<String, HashSet<String>>,
}

impl Default for TaskGraph {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::autonomy::task::{RetryPolicy, TaskContext};

    fn test_task(id: &str, deps: Vec<&str>) -> Task {
        Task {
            id: id.to_string(),
            title: id.to_string(),
            description: "".to_string(),
            status: TaskStatus::Pending,
            priority: TaskPriority::Medium,
            complexity: ComplexityLevel::Simple,
            estimated_duration_secs: 60,
            actual_duration_secs: None,
            assignee: None,
            executor_pid: None,
            dependencies: deps.into_iter().map(|s| s.to_string()).collect(),
            dependents: Vec::new(),
            plan_id: "plan_1".to_string(),
            parent_task_id: None,
            sub_tasks: Vec::new(),
            context: TaskContext::default(),
            result: None,
            retry_policy: RetryPolicy::default(),
            retry_count: 0,
            next_retry_at_ms: None,
            created_at_ms: 0,
            started_at_ms: None,
            completed_at_ms: None,
        }
    }

    #[test]
    fn add_task_and_complete_unblocks_dependents() {
        let mut graph = TaskGraph::new();
        graph.add_task(test_task("a", vec![])).unwrap();
        graph.add_task(test_task("b", vec!["a"])).unwrap();

        // Pop "a" from ready queue (simulating scheduling)
        let popped = graph.pop_ready();
        assert_eq!(popped.map(|r| r.task_id), Some("a".to_string()));

        let ready = graph.complete_task("a").unwrap();
        assert!(ready.contains(&"b".to_string()));
        assert_eq!(graph.ready_queue_len(), 1);
    }

    #[test]
    fn circular_dependency_detection() {
        let mut graph = TaskGraph::new();
        graph.add_task(test_task("a", vec!["c"])).unwrap();
        graph.add_task(test_task("b", vec!["a"])).unwrap();
        graph.add_task(test_task("c", vec!["b"])).unwrap();

        let cycle = graph.has_circular_dependency();
        assert!(cycle.is_some());
    }
}
