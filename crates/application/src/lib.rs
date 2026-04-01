use domain::HealthStatus;

#[derive(Clone, Copy, Default)]
pub struct HealthService;

impl HealthService {
    pub fn get_status(&self) -> HealthStatus {
        HealthStatus::default()
    }
}
