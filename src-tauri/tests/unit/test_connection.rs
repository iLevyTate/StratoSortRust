use stratosort::ai::connection::{ConnectionPool, ConnectionManager, ConnectionStatus};
use stratosort::error::Result;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;

#[cfg(test)]
mod connection_tests {
    use super::*;

    // Mock connection for testing
    #[derive(Clone)]
    struct MockConnection {
        id: String,
        healthy: bool,
    }

    impl MockConnection {
        fn new(id: &str, healthy: bool) -> Self {
            Self {
                id: id.to_string(),
                healthy,
            }
        }

        async fn health_check(&self) -> bool {
            self.healthy
        }
    }

    #[tokio::test]
    async fn test_connection_pool_creation() {
        let pool = ConnectionPool::<MockConnection>::new(5, Duration::from_secs(30));
        
        assert_eq!(pool.max_size(), 5);
        assert_eq!(pool.current_size(), 0);
        assert!(pool.is_empty());
    }

    #[tokio::test]
    async fn test_connection_pool_add_connection() {
        let pool = ConnectionPool::<MockConnection>::new(5, Duration::from_secs(30));
        let conn = MockConnection::new("conn1", true);
        
        pool.add(conn.clone()).await;
        
        assert_eq!(pool.current_size(), 1);
        assert!(!pool.is_empty());
    }

    #[tokio::test]
    async fn test_connection_pool_get_connection() {
        let pool = ConnectionPool::<MockConnection>::new(5, Duration::from_secs(30));
        let conn = MockConnection::new("conn1", true);
        
        pool.add(conn.clone()).await;
        
        let retrieved = pool.get().await;
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().id, "conn1");
    }

    #[tokio::test]
    async fn test_connection_pool_round_robin() {
        let pool = ConnectionPool::<MockConnection>::new(5, Duration::from_secs(30));
        
        pool.add(MockConnection::new("conn1", true)).await;
        pool.add(MockConnection::new("conn2", true)).await;
        pool.add(MockConnection::new("conn3", true)).await;
        
        // Should cycle through connections
        assert_eq!(pool.get().await.unwrap().id, "conn1");
        assert_eq!(pool.get().await.unwrap().id, "conn2");
        assert_eq!(pool.get().await.unwrap().id, "conn3");
        assert_eq!(pool.get().await.unwrap().id, "conn1"); // Back to first
    }

    #[tokio::test]
    async fn test_connection_pool_max_size() {
        let pool = ConnectionPool::<MockConnection>::new(2, Duration::from_secs(30));
        
        pool.add(MockConnection::new("conn1", true)).await;
        pool.add(MockConnection::new("conn2", true)).await;
        pool.add(MockConnection::new("conn3", true)).await; // Should not be added
        
        assert_eq!(pool.current_size(), 2);
    }

    #[tokio::test]
    async fn test_connection_pool_remove() {
        let pool = ConnectionPool::<MockConnection>::new(5, Duration::from_secs(30));
        
        let conn1 = MockConnection::new("conn1", true);
        let conn2 = MockConnection::new("conn2", true);
        
        pool.add(conn1.clone()).await;
        pool.add(conn2.clone()).await;
        
        assert_eq!(pool.current_size(), 2);
        
        pool.remove(&conn1).await;
        
        assert_eq!(pool.current_size(), 1);
        assert_eq!(pool.get().await.unwrap().id, "conn2");
    }

    #[tokio::test]
    async fn test_connection_pool_clear() {
        let pool = ConnectionPool::<MockConnection>::new(5, Duration::from_secs(30));
        
        pool.add(MockConnection::new("conn1", true)).await;
        pool.add(MockConnection::new("conn2", true)).await;
        pool.add(MockConnection::new("conn3", true)).await;
        
        assert_eq!(pool.current_size(), 3);
        
        pool.clear().await;
        
        assert_eq!(pool.current_size(), 0);
        assert!(pool.is_empty());
    }

    #[tokio::test]
    async fn test_connection_pool_health_check() {
        let pool = ConnectionPool::<MockConnection>::new(5, Duration::from_secs(30));
        
        pool.add(MockConnection::new("conn1", true)).await;
        pool.add(MockConnection::new("conn2", false)).await; // Unhealthy
        pool.add(MockConnection::new("conn3", true)).await;
        
        let health_status = pool.health_check().await;
        
        assert_eq!(health_status.total, 3);
        assert_eq!(health_status.healthy, 2);
        assert_eq!(health_status.unhealthy, 1);
    }

    #[tokio::test]
    async fn test_connection_pool_remove_unhealthy() {
        let pool = ConnectionPool::<MockConnection>::new(5, Duration::from_secs(30));
        
        pool.add(MockConnection::new("conn1", true)).await;
        pool.add(MockConnection::new("conn2", false)).await; // Unhealthy
        pool.add(MockConnection::new("conn3", true)).await;
        pool.add(MockConnection::new("conn4", false)).await; // Unhealthy
        
        assert_eq!(pool.current_size(), 4);
        
        pool.remove_unhealthy().await;
        
        assert_eq!(pool.current_size(), 2);
        
        // Only healthy connections should remain
        assert_eq!(pool.get().await.unwrap().id, "conn1");
        assert_eq!(pool.get().await.unwrap().id, "conn3");
    }

    #[tokio::test]
    async fn test_connection_manager_creation() {
        let manager = ConnectionManager::<MockConnection>::new(
            10,
            Duration::from_secs(60),
            Duration::from_secs(5)
        );
        
        assert_eq!(manager.pool_size(), 0);
    }

    #[tokio::test]
    async fn test_connection_manager_add_and_get() {
        let manager = ConnectionManager::<MockConnection>::new(
            10,
            Duration::from_secs(60),
            Duration::from_secs(5)
        );
        
        let conn = MockConnection::new("managed1", true);
        manager.add_connection(conn.clone()).await;
        
        assert_eq!(manager.pool_size(), 1);
        
        let retrieved = manager.get_connection().await;
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().id, "managed1");
    }

    #[tokio::test]
    async fn test_connection_manager_reconnect() {
        let manager = ConnectionManager::<MockConnection>::new(
            10,
            Duration::from_secs(60),
            Duration::from_secs(5)
        );
        
        manager.add_connection(MockConnection::new("old1", true)).await;
        manager.add_connection(MockConnection::new("old2", true)).await;
        
        assert_eq!(manager.pool_size(), 2);
        
        // Simulate reconnect by clearing and adding new connections
        manager.reconnect(vec![
            MockConnection::new("new1", true),
            MockConnection::new("new2", true),
            MockConnection::new("new3", true),
        ]).await;
        
        assert_eq!(manager.pool_size(), 3);
        
        // Should have new connections
        let conn = manager.get_connection().await.unwrap();
        assert!(conn.id.starts_with("new"));
    }

    #[tokio::test]
    async fn test_connection_manager_status() {
        let manager = ConnectionManager::<MockConnection>::new(
            10,
            Duration::from_secs(60),
            Duration::from_secs(5)
        );
        
        manager.add_connection(MockConnection::new("conn1", true)).await;
        manager.add_connection(MockConnection::new("conn2", false)).await;
        manager.add_connection(MockConnection::new("conn3", true)).await;
        
        let status = manager.get_status().await;
        
        assert_eq!(status.total_connections, 3);
        assert_eq!(status.healthy_connections, 2);
        assert_eq!(status.unhealthy_connections, 1);
        assert!(status.last_health_check.is_some());
    }

    #[tokio::test]
    async fn test_connection_pool_concurrent_access() {
        let pool = Arc::new(ConnectionPool::<MockConnection>::new(5, Duration::from_secs(30)));
        
        for i in 0..5 {
            pool.add(MockConnection::new(&format!("conn{}", i), true)).await;
        }
        
        let mut handles = vec![];
        
        // Spawn multiple tasks accessing the pool
        for _ in 0..10 {
            let pool_clone = pool.clone();
            let handle = tokio::spawn(async move {
                for _ in 0..5 {
                    let conn = pool_clone.get().await;
                    assert!(conn.is_some());
                    sleep(Duration::from_millis(10)).await;
                }
            });
            handles.push(handle);
        }
        
        // All tasks should complete successfully
        for handle in handles {
            assert!(handle.await.is_ok());
        }
    }

    #[tokio::test]
    async fn test_connection_pool_empty_pool() {
        let pool = ConnectionPool::<MockConnection>::new(5, Duration::from_secs(30));
        
        // Getting from empty pool should return None
        assert!(pool.get().await.is_none());
        assert!(pool.is_empty());
    }

    #[tokio::test]
    async fn test_connection_manager_auto_cleanup() {
        let manager = ConnectionManager::<MockConnection>::new(
            10,
            Duration::from_secs(60),
            Duration::from_millis(100) // Short interval for testing
        );
        
        // Add mix of healthy and unhealthy connections
        manager.add_connection(MockConnection::new("healthy1", true)).await;
        manager.add_connection(MockConnection::new("unhealthy1", false)).await;
        manager.add_connection(MockConnection::new("healthy2", true)).await;
        manager.add_connection(MockConnection::new("unhealthy2", false)).await;
        
        assert_eq!(manager.pool_size(), 4);
        
        // Start cleanup task
        let manager_clone = manager.clone();
        let cleanup_handle = tokio::spawn(async move {
            manager_clone.start_cleanup_task().await;
        });
        
        // Wait for cleanup to run
        sleep(Duration::from_millis(200)).await;
        
        // Unhealthy connections should be removed
        assert_eq!(manager.pool_size(), 2);
        
        cleanup_handle.abort();
    }

    #[tokio::test]
    async fn test_connection_status_serialization() {
        use serde_json;
        
        let status = ConnectionStatus {
            total_connections: 10,
            healthy_connections: 8,
            unhealthy_connections: 2,
            last_health_check: Some(chrono::Utc::now()),
            pool_utilization: 0.8,
        };
        
        // Should serialize to JSON
        let json = serde_json::to_string(&status);
        assert!(json.is_ok());
        
        let json_str = json.unwrap();
        assert!(json_str.contains("total_connections"));
        assert!(json_str.contains("healthy_connections"));
        
        // Should deserialize back
        let deserialized: Result<ConnectionStatus, _> = serde_json::from_str(&json_str);
        assert!(deserialized.is_ok());
    }

    #[tokio::test]
    async fn test_connection_pool_with_zero_max_size() {
        let pool = ConnectionPool::<MockConnection>::new(0, Duration::from_secs(30));
        
        // Should not add any connections
        pool.add(MockConnection::new("conn1", true)).await;
        
        assert_eq!(pool.current_size(), 0);
        assert!(pool.is_empty());
    }

    #[tokio::test]
    async fn test_connection_manager_metrics() {
        let manager = ConnectionManager::<MockConnection>::new(
            5,
            Duration::from_secs(60),
            Duration::from_secs(5)
        );
        
        // Fill pool to capacity
        for i in 0..5 {
            manager.add_connection(MockConnection::new(&format!("conn{}", i), true)).await;
        }
        
        let status = manager.get_status().await;
        assert_eq!(status.pool_utilization, 1.0); // 100% utilized
        
        // Remove some connections
        manager.reconnect(vec![
            MockConnection::new("new1", true),
            MockConnection::new("new2", true),
        ]).await;
        
        let status = manager.get_status().await;
        assert_eq!(status.pool_utilization, 0.4); // 40% utilized (2/5)
    }
}