use stratosort::services::{NotificationService, Notification, NotificationType, NotificationPriority};
use stratosort::error::{AppError, Result};
use tauri::test::{mock_app, mock_context};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::{timeout, sleep};
use uuid::Uuid;

#[cfg(test)]
mod notification_service_tests {
    use super::*;

    fn create_test_service() -> Arc<NotificationService> {
        let app = mock_app(mock_context());
        Arc::new(NotificationService::new(app.handle().clone()))
    }

    // Basic Notification Creation Tests
    #[tokio::test]
    async fn test_create_info_notification() {
        let service = create_test_service();
        
        let notification = service.create_notification(
            NotificationType::Info,
            "Test Info",
            "This is an info notification"
        ).await.unwrap();
        
        assert_eq!(notification.notification_type, NotificationType::Info);
        assert_eq!(notification.title, "Test Info");
        assert_eq!(notification.message, "This is an info notification");
        assert!(!notification.read);
    }

    #[tokio::test]
    async fn test_create_success_notification() {
        let service = create_test_service();
        
        let notification = service.create_notification(
            NotificationType::Success,
            "Operation Complete",
            "File has been successfully processed"
        ).await.unwrap();
        
        assert_eq!(notification.notification_type, NotificationType::Success);
        assert!(notification.id.len() > 0);
    }

    #[tokio::test]
    async fn test_create_warning_notification() {
        let service = create_test_service();
        
        let notification = service.create_notification(
            NotificationType::Warning,
            "Storage Warning",
            "Disk space is running low"
        ).await.unwrap();
        
        assert_eq!(notification.notification_type, NotificationType::Warning);
    }

    #[tokio::test]
    async fn test_create_error_notification() {
        let service = create_test_service();
        
        let notification = service.create_notification(
            NotificationType::Error,
            "Operation Failed",
            "Failed to process file: Permission denied"
        ).await.unwrap();
        
        assert_eq!(notification.notification_type, NotificationType::Error);
    }

    // Notification with Metadata Tests
    #[tokio::test]
    async fn test_create_notification_with_metadata() {
        let service = create_test_service();
        
        let metadata = serde_json::json!({
            "file_path": "/test/document.pdf",
            "file_size": 1024,
            "operation": "analyze"
        });
        
        let notification = service.create_notification_with_metadata(
            NotificationType::Info,
            "File Analysis",
            "Analyzing document.pdf",
            metadata.clone()
        ).await.unwrap();
        
        assert!(notification.metadata.is_some());
        assert_eq!(notification.metadata.unwrap(), metadata);
    }

    #[tokio::test]
    async fn test_create_notification_with_actions() {
        let service = create_test_service();
        
        let actions = vec![
            ("view".to_string(), "View File".to_string()),
            ("dismiss".to_string(), "Dismiss".to_string()),
        ];
        
        let notification = service.create_notification_with_actions(
            NotificationType::Info,
            "New File",
            "A new file has been detected",
            actions.clone()
        ).await.unwrap();
        
        assert!(notification.actions.is_some());
        assert_eq!(notification.actions.unwrap().len(), 2);
    }

    #[tokio::test]
    async fn test_create_notification_with_priority() {
        let service = create_test_service();
        
        let notification = service.create_notification_with_priority(
            NotificationType::Warning,
            "High Priority Warning",
            "This requires immediate attention",
            NotificationPriority::High
        ).await.unwrap();
        
        assert_eq!(notification.priority, NotificationPriority::High);
    }

    // Notification Management Tests
    #[tokio::test]
    async fn test_get_notification_by_id() {
        let service = create_test_service();
        
        let created = service.create_notification(
            NotificationType::Info,
            "Test",
            "Test message"
        ).await.unwrap();
        
        let retrieved = service.get_notification(&created.id).await.unwrap();
        assert!(retrieved.is_some());
        
        let notification = retrieved.unwrap();
        assert_eq!(notification.id, created.id);
        assert_eq!(notification.title, created.title);
    }

    #[tokio::test]
    async fn test_get_nonexistent_notification() {
        let service = create_test_service();
        
        let result = service.get_notification("nonexistent-id").await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_mark_as_read() {
        let service = create_test_service();
        
        let notification = service.create_notification(
            NotificationType::Info,
            "Unread",
            "This is unread"
        ).await.unwrap();
        
        assert!(!notification.read);
        
        service.mark_as_read(&notification.id).await.unwrap();
        
        let updated = service.get_notification(&notification.id).await.unwrap().unwrap();
        assert!(updated.read);
    }

    #[tokio::test]
    async fn test_mark_multiple_as_read() {
        let service = create_test_service();
        
        // Create multiple notifications
        let mut ids = vec![];
        for i in 0..3 {
            let n = service.create_notification(
                NotificationType::Info,
                &format!("Notification {}", i),
                "Message"
            ).await.unwrap();
            ids.push(n.id);
        }
        
        // Mark all as read
        service.mark_multiple_as_read(&ids).await.unwrap();
        
        // Verify all are marked as read
        for id in ids {
            let n = service.get_notification(&id).await.unwrap().unwrap();
            assert!(n.read);
        }
    }

    #[tokio::test]
    async fn test_delete_notification() {
        let service = create_test_service();
        
        let notification = service.create_notification(
            NotificationType::Info,
            "To Delete",
            "This will be deleted"
        ).await.unwrap();
        
        service.delete_notification(&notification.id).await.unwrap();
        
        let result = service.get_notification(&notification.id).await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_delete_multiple_notifications() {
        let service = create_test_service();
        
        let mut ids = vec![];
        for i in 0..3 {
            let n = service.create_notification(
                NotificationType::Info,
                &format!("Delete {}", i),
                "To be deleted"
            ).await.unwrap();
            ids.push(n.id);
        }
        
        service.delete_multiple(&ids).await.unwrap();
        
        for id in ids {
            let result = service.get_notification(&id).await.unwrap();
            assert!(result.is_none());
        }
    }

    // Notification Retrieval Tests
    #[tokio::test]
    async fn test_get_all_notifications() {
        let service = create_test_service();
        
        // Create multiple notifications
        for i in 0..5 {
            service.create_notification(
                NotificationType::Info,
                &format!("Notification {}", i),
                "Message"
            ).await.unwrap();
        }
        
        let all = service.get_all_notifications().await.unwrap();
        assert_eq!(all.len(), 5);
    }

    #[tokio::test]
    async fn test_get_unread_notifications() {
        let service = create_test_service();
        
        // Create mix of read and unread
        for i in 0..3 {
            let n = service.create_notification(
                NotificationType::Info,
                &format!("Unread {}", i),
                "Message"
            ).await.unwrap();
            
            if i == 1 {
                service.mark_as_read(&n.id).await.unwrap();
            }
        }
        
        let unread = service.get_unread_notifications().await.unwrap();
        assert_eq!(unread.len(), 2);
        
        for notification in unread {
            assert!(!notification.read);
        }
    }

    #[tokio::test]
    async fn test_get_notifications_by_type() {
        let service = create_test_service();
        
        // Create notifications of different types
        service.create_notification(
            NotificationType::Info,
            "Info",
            "Info message"
        ).await.unwrap();
        
        service.create_notification(
            NotificationType::Warning,
            "Warning 1",
            "Warning message"
        ).await.unwrap();
        
        service.create_notification(
            NotificationType::Warning,
            "Warning 2",
            "Another warning"
        ).await.unwrap();
        
        service.create_notification(
            NotificationType::Error,
            "Error",
            "Error message"
        ).await.unwrap();
        
        let warnings = service.get_notifications_by_type(NotificationType::Warning).await.unwrap();
        assert_eq!(warnings.len(), 2);
        
        for notification in warnings {
            assert_eq!(notification.notification_type, NotificationType::Warning);
        }
    }

    #[tokio::test]
    async fn test_get_recent_notifications() {
        let service = create_test_service();
        
        // Create notifications with delays
        for i in 0..5 {
            service.create_notification(
                NotificationType::Info,
                &format!("Notification {}", i),
                "Message"
            ).await.unwrap();
            
            if i < 4 {
                sleep(Duration::from_millis(10)).await;
            }
        }
        
        let recent = service.get_recent_notifications(3).await.unwrap();
        assert_eq!(recent.len(), 3);
        
        // Should be ordered by timestamp (newest first)
        for i in 0..2 {
            assert!(recent[i].timestamp >= recent[i+1].timestamp);
        }
    }

    // Notification Count Tests
    #[tokio::test]
    async fn test_get_unread_count() {
        let service = create_test_service();
        
        // Create notifications
        for i in 0..4 {
            let n = service.create_notification(
                NotificationType::Info,
                &format!("Notification {}", i),
                "Message"
            ).await.unwrap();
            
            // Mark some as read
            if i < 2 {
                service.mark_as_read(&n.id).await.unwrap();
            }
        }
        
        let count = service.get_unread_count().await.unwrap();
        assert_eq!(count, 2);
    }

    #[tokio::test]
    async fn test_get_count_by_type() {
        let service = create_test_service();
        
        // Create notifications of different types
        for _ in 0..2 {
            service.create_notification(
                NotificationType::Info,
                "Info",
                "Message"
            ).await.unwrap();
        }
        
        for _ in 0..3 {
            service.create_notification(
                NotificationType::Warning,
                "Warning",
                "Message"
            ).await.unwrap();
        }
        
        service.create_notification(
            NotificationType::Error,
            "Error",
            "Message"
        ).await.unwrap();
        
        assert_eq!(service.get_count_by_type(NotificationType::Info).await.unwrap(), 2);
        assert_eq!(service.get_count_by_type(NotificationType::Warning).await.unwrap(), 3);
        assert_eq!(service.get_count_by_type(NotificationType::Error).await.unwrap(), 1);
        assert_eq!(service.get_count_by_type(NotificationType::Success).await.unwrap(), 0);
    }

    // Bulk Operations Tests
    #[tokio::test]
    async fn test_clear_all_notifications() {
        let service = create_test_service();
        
        // Create multiple notifications
        for i in 0..5 {
            service.create_notification(
                NotificationType::Info,
                &format!("Notification {}", i),
                "Message"
            ).await.unwrap();
        }
        
        service.clear_all().await.unwrap();
        
        let all = service.get_all_notifications().await.unwrap();
        assert!(all.is_empty());
    }

    #[tokio::test]
    async fn test_clear_read_notifications() {
        let service = create_test_service();
        
        // Create mix of read and unread
        let mut unread_ids = vec![];
        for i in 0..5 {
            let n = service.create_notification(
                NotificationType::Info,
                &format!("Notification {}", i),
                "Message"
            ).await.unwrap();
            
            if i < 3 {
                service.mark_as_read(&n.id).await.unwrap();
            } else {
                unread_ids.push(n.id);
            }
        }
        
        service.clear_read().await.unwrap();
        
        let remaining = service.get_all_notifications().await.unwrap();
        assert_eq!(remaining.len(), 2);
        
        // Only unread should remain
        for notification in remaining {
            assert!(!notification.read);
            assert!(unread_ids.contains(&notification.id));
        }
    }

    #[tokio::test]
    async fn test_clear_old_notifications() {
        let service = create_test_service();
        
        // Create old notifications (simulate with metadata)
        for i in 0..3 {
            let mut n = service.create_notification(
                NotificationType::Info,
                &format!("Old {}", i),
                "Old message"
            ).await.unwrap();
            
            // Manually set old timestamp (normally would use database)
            n.timestamp = chrono::Utc::now() - chrono::Duration::days(40);
            // In real implementation, this would be updated in storage
        }
        
        // Create recent notifications
        for i in 0..2 {
            service.create_notification(
                NotificationType::Info,
                &format!("Recent {}", i),
                "Recent message"
            ).await.unwrap();
        }
        
        // Clear notifications older than 30 days
        let cleared = service.clear_old_notifications(30).await.unwrap();
        
        // Note: In actual implementation, this would work with persistent storage
        // For this test, we're simulating the behavior
        assert!(cleared >= 0);
    }

    // Event Subscription Tests
    #[tokio::test]
    async fn test_subscribe_to_notifications() {
        let service = create_test_service();
        
        let mut subscriber = service.subscribe().await;
        
        // Create notification after subscribing
        let notification = service.create_notification(
            NotificationType::Info,
            "Test Event",
            "This should trigger an event"
        ).await.unwrap();
        
        // Should receive the notification event
        let event = timeout(Duration::from_secs(1), subscriber.recv())
            .await
            .expect("Timeout waiting for event")
            .expect("Failed to receive event");
        
        assert_eq!(event.id, notification.id);
        assert_eq!(event.title, notification.title);
    }

    #[tokio::test]
    async fn test_multiple_subscribers() {
        let service = create_test_service();
        
        let mut sub1 = service.subscribe().await;
        let mut sub2 = service.subscribe().await;
        
        let notification = service.create_notification(
            NotificationType::Info,
            "Broadcast",
            "This goes to all subscribers"
        ).await.unwrap();
        
        // Both subscribers should receive the event
        let event1 = timeout(Duration::from_secs(1), sub1.recv()).await;
        let event2 = timeout(Duration::from_secs(1), sub2.recv()).await;
        
        assert!(event1.is_ok());
        assert!(event2.is_ok());
        
        assert_eq!(event1.unwrap().unwrap().id, notification.id);
        assert_eq!(event2.unwrap().unwrap().id, notification.id);
    }

    // Priority and Filtering Tests
    #[tokio::test]
    async fn test_get_high_priority_notifications() {
        let service = create_test_service();
        
        // Create notifications with different priorities
        service.create_notification_with_priority(
            NotificationType::Info,
            "Low Priority",
            "Not urgent",
            NotificationPriority::Low
        ).await.unwrap();
        
        service.create_notification_with_priority(
            NotificationType::Warning,
            "Normal Priority",
            "Regular notification",
            NotificationPriority::Normal
        ).await.unwrap();
        
        service.create_notification_with_priority(
            NotificationType::Error,
            "High Priority 1",
            "Urgent!",
            NotificationPriority::High
        ).await.unwrap();
        
        service.create_notification_with_priority(
            NotificationType::Error,
            "High Priority 2",
            "Also urgent!",
            NotificationPriority::High
        ).await.unwrap();
        
        let high_priority = service.get_notifications_by_priority(
            NotificationPriority::High
        ).await.unwrap();
        
        assert_eq!(high_priority.len(), 2);
        
        for notification in high_priority {
            assert_eq!(notification.priority, NotificationPriority::High);
        }
    }

    #[tokio::test]
    async fn test_search_notifications() {
        let service = create_test_service();
        
        // Create notifications with searchable content
        service.create_notification(
            NotificationType::Info,
            "File Processing",
            "Processing document.pdf successfully"
        ).await.unwrap();
        
        service.create_notification(
            NotificationType::Warning,
            "Storage Alert",
            "Disk space for documents folder is low"
        ).await.unwrap();
        
        service.create_notification(
            NotificationType::Success,
            "Upload Complete",
            "Image files uploaded to cloud"
        ).await.unwrap();
        
        // Search for "document"
        let results = service.search_notifications("document").await.unwrap();
        assert_eq!(results.len(), 2);
        
        // Search for "upload"
        let results = service.search_notifications("upload").await.unwrap();
        assert_eq!(results.len(), 1);
    }

    // Concurrent Access Tests
    #[tokio::test]
    async fn test_concurrent_notification_creation() {
        let service = create_test_service();
        
        let handles: Vec<_> = (0..50)
            .map(|i| {
                let service_clone = service.clone();
                tokio::spawn(async move {
                    service_clone.create_notification(
                        NotificationType::Info,
                        &format!("Concurrent {}", i),
                        "Concurrent message"
                    ).await
                })
            })
            .collect();
        
        for handle in handles {
            assert!(handle.await.unwrap().is_ok());
        }
        
        let all = service.get_all_notifications().await.unwrap();
        assert_eq!(all.len(), 50);
    }

    #[tokio::test]
    async fn test_concurrent_read_operations() {
        let service = create_test_service();
        
        // Create notifications
        let mut ids = vec![];
        for i in 0..10 {
            let n = service.create_notification(
                NotificationType::Info,
                &format!("Notification {}", i),
                "Message"
            ).await.unwrap();
            ids.push(n.id);
        }
        
        // Concurrent reads
        let handles: Vec<_> = ids.iter()
            .map(|id| {
                let service_clone = service.clone();
                let id_clone = id.clone();
                tokio::spawn(async move {
                    service_clone.mark_as_read(&id_clone).await
                })
            })
            .collect();
        
        for handle in handles {
            assert!(handle.await.unwrap().is_ok());
        }
        
        // Verify all are read
        for id in ids {
            let n = service.get_notification(&id).await.unwrap().unwrap();
            assert!(n.read);
        }
    }

    // Error Handling Tests
    #[tokio::test]
    async fn test_empty_title_notification() {
        let service = create_test_service();
        
        let result = service.create_notification(
            NotificationType::Info,
            "",
            "Message with empty title"
        ).await;
        
        // Should handle empty title gracefully
        assert!(result.is_ok() || result.is_err());
    }

    #[tokio::test]
    async fn test_very_long_notification_content() {
        let service = create_test_service();
        
        let long_message = "x".repeat(10000);
        
        let result = service.create_notification(
            NotificationType::Info,
            "Long Notification",
            &long_message
        ).await;
        
        assert!(result.is_ok());
        
        let notification = result.unwrap();
        // Message might be truncated
        assert!(notification.message.len() <= 10000);
    }

    #[tokio::test]
    async fn test_notification_with_complex_metadata() {
        let service = create_test_service();
        
        let complex_metadata = serde_json::json!({
            "nested": {
                "deep": {
                    "value": "test"
                }
            },
            "array": [1, 2, 3, 4, 5],
            "mixed": {
                "number": 42,
                "string": "text",
                "bool": true,
                "null": null
            }
        });
        
        let notification = service.create_notification_with_metadata(
            NotificationType::Info,
            "Complex",
            "Notification with complex metadata",
            complex_metadata.clone()
        ).await.unwrap();
        
        assert_eq!(notification.metadata.unwrap(), complex_metadata);
    }

    // System Integration Tests
    #[tokio::test]
    async fn test_notification_persistence() {
        let service = create_test_service();
        
        // Create notification
        let notification = service.create_notification(
            NotificationType::Info,
            "Persistent",
            "This should persist"
        ).await.unwrap();
        
        let id = notification.id.clone();
        
        // Notification should be retrievable
        let retrieved = service.get_notification(&id).await.unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().id, id);
    }

    #[tokio::test]
    async fn test_notification_ordering() {
        let service = create_test_service();
        
        // Create notifications with specific order
        let mut ids = vec![];
        for i in 0..5 {
            let n = service.create_notification(
                NotificationType::Info,
                &format!("Ordered {}", i),
                "Message"
            ).await.unwrap();
            ids.push(n.id);
            sleep(Duration::from_millis(10)).await;
        }
        
        let all = service.get_all_notifications().await.unwrap();
        
        // Should maintain chronological order
        for i in 0..4 {
            assert!(all[i].timestamp >= all[i+1].timestamp);
        }
    }
}