use redis::{AsyncCommands, RedisResult, Script};
use std::time::{SystemTime, UNIX_EPOCH};
use anyhow::Result;
use tracing::info;

#[derive(Clone)]
pub struct RedisManager {
    client: redis::Client,
}

impl RedisManager {
    pub async fn new(redis_url: &str) -> Result<Self> {
        let client = redis::Client::open(redis_url)?;
        // Test connection
        let mut conn = client.get_async_connection().await?;
        let _: () = redis::cmd("PING").query_async(&mut conn).await?;
        info!("Successfully connected to Redis at {}", redis_url);
        Ok(Self { client })
    }

    pub async fn get_async_connection(&self) -> RedisResult<redis::aio::MultiplexedConnection> {
        self.client.get_async_connection().await
    }
}


// Rate limiting logic (simplified version of Upstash's ratelimit/fixed-window)
// This example uses a fixed window algorithm.
pub struct RateLimiter {
    redis_conn_manager: RedisManager, // Using manager to get connections
    limit: u32,      // Max requests per window
    window_secs: u32, // Window size in seconds
    prefix: String,   // Prefix for Redis keys
}

#[derive(Debug)]
pub struct RateLimitResponse {
    pub success: bool,
    pub limit: u32,
    pub remaining: u32,
    pub reset: u64, // Timestamp in ms when the limit resets
}

impl RateLimiter {
    pub fn new(redis_conn_manager: RedisManager, prefix: &str, limit: u32, window_secs: u32) -> Self {
        RateLimiter {
            redis_conn_manager,
            limit,
            window_secs,
            prefix: prefix.to_string(),
        }
    }

    pub async fn limit(&self, identifier: &str) -> Result<RateLimitResponse> {
        let mut conn = self.redis_conn_manager.get_async_connection().await?;
        let key = format!("{}:{}", self.prefix, identifier);

        let now_ms = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis() as u64;

        // LUA script for atomicity (Fixed Window Algorithm)
        // Inspired by https://redis.io/commands/incr/#pattern-rate-limiter-1
        // and Upstash ratelimit library's approach.
        let script = Script::new(r"
            local key = KEYS[1]
            local limit = tonumber(ARGV[1])
            local window_ms = tonumber(ARGV[2])
            local now_ms = tonumber(ARGV[3])

            local current_window_start_ms = math.floor(now_ms / window_ms) * window_ms
            local redis_key = key .. ':' .. current_window_start_ms

            local count = redis.call('INCR', redis_key)
            if count == 1 then
                redis.call('PEXPIRE', redis_key, window_ms)
            end

            local remaining = limit - count
            if remaining < 0 then
                remaining = 0
            end

            return {count, remaining, current_window_start_ms + window_ms}
        ");

        let result: Vec<i64> = script
            .key(&key) // Note: The script constructs the final key with timestamp
            .arg(self.limit as i64)
            .arg(self.window_secs as i64 * 1000) // window in ms
            .arg(now_ms as i64)
            .invoke_async(&mut conn)
            .await?;

        let count = result[0] as u32;
        let remaining = result[1] as u32;
        let reset_ts_ms = result[2] as u64;

        Ok(RateLimitResponse {
            success: count <= self.limit,
            limit: self.limit,
            remaining,
            reset: reset_ts_ms,
        })
    }
}
