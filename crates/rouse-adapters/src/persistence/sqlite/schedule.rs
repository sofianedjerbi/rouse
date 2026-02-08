use async_trait::async_trait;

use rouse_core::schedule::Schedule;
use rouse_ports::error::PortError;
use rouse_ports::outbound::ScheduleRepository;

use super::SqliteDb;

#[async_trait]
impl ScheduleRepository for SqliteDb {
    async fn save(&self, schedule: &Schedule) -> Result<(), PortError> {
        let id = schedule.id().to_string();
        let data =
            serde_json::to_string(schedule).map_err(|e| PortError::Persistence(e.to_string()))?;

        sqlx::query(
            "INSERT INTO schedules (id, data) VALUES (?, ?)
             ON CONFLICT(id) DO UPDATE SET data = excluded.data",
        )
        .bind(&id)
        .bind(&data)
        .execute(&self.pool)
        .await
        .map_err(|e| PortError::Persistence(e.to_string()))?;

        Ok(())
    }

    async fn find_by_id(&self, id: &str) -> Result<Option<Schedule>, PortError> {
        let row: Option<(String,)> = sqlx::query_as("SELECT data FROM schedules WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| PortError::Persistence(e.to_string()))?;

        match row {
            Some((data,)) => {
                let schedule: Schedule = serde_json::from_str(&data)
                    .map_err(|e| PortError::Persistence(e.to_string()))?;
                Ok(Some(schedule))
            }
            None => Ok(None),
        }
    }

    async fn list_all(&self) -> Result<Vec<Schedule>, PortError> {
        let rows: Vec<(String,)> = sqlx::query_as("SELECT data FROM schedules")
            .fetch_all(&self.pool)
            .await
            .map_err(|e| PortError::Persistence(e.to_string()))?;

        let mut schedules = Vec::with_capacity(rows.len());
        for (data,) in rows {
            let schedule: Schedule =
                serde_json::from_str(&data).map_err(|e| PortError::Persistence(e.to_string()))?;
            schedules.push(schedule);
        }
        Ok(schedules)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rouse_core::ids::UserId;
    use rouse_core::schedule::{HandoffTime, Rotation};

    async fn db() -> SqliteDb {
        SqliteDb::new("sqlite::memory:").await.unwrap()
    }

    fn make_schedule(name: &str) -> Schedule {
        Schedule::new(
            name.into(),
            "Europe/Zurich".parse().unwrap(),
            Rotation::Weekly,
            vec![UserId::new(), UserId::new()],
            HandoffTime {
                day: chrono::Weekday::Mon,
                hour: 9,
                minute: 0,
            },
        )
        .unwrap()
    }

    #[tokio::test]
    async fn save_and_find_by_id() {
        let db = db().await;
        let sched = make_schedule("platform");
        let id = sched.id().to_string();

        db.save(&sched).await.unwrap();

        let found = db.find_by_id(&id).await.unwrap().unwrap();
        assert_eq!(found.name(), "platform");
        assert_eq!(found.participants().len(), 2);
    }

    #[tokio::test]
    async fn list_all_returns_saved() {
        let db = db().await;
        db.save(&make_schedule("team-a")).await.unwrap();
        db.save(&make_schedule("team-b")).await.unwrap();

        let all = db.list_all().await.unwrap();
        assert_eq!(all.len(), 2);
    }
}
