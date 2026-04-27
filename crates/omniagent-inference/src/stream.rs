// OmniAgent OS Phase 10: 流式推理
// 支持流式响应的会话管理和块传输

use std::collections::HashMap;

use crate::types::{InferenceError, ModelId, StreamChunk};

/// 流式推理会话，管理一个流式推理请求的完整生命周期
pub struct StreamSession {
    /// 会话 ID
    pub id: u64,
    /// 使用的模型 ID
    pub model_id: ModelId,
    /// 已接收的块列表
    pub chunks: Vec<StreamChunk>,
    /// 总 Token 数
    pub total_tokens: u32,
    /// 会话是否已完成
    pub is_complete: bool,
    /// 创建时间戳
    pub created_at: u64,
}

impl StreamSession {
    /// 创建新的流式推理会话
    pub fn new(id: u64, model_id: ModelId) -> Self {
        Self {
            id,
            model_id,
            chunks: Vec::new(),
            total_tokens: 0,
            is_complete: false,
            created_at: 0,
        }
    }

    /// 创建带时间戳的流式推理会话
    pub fn new_with_timestamp(id: u64, model_id: ModelId, created_at: u64) -> Self {
        Self {
            id,
            model_id,
            chunks: Vec::new(),
            total_tokens: 0,
            is_complete: false,
            created_at,
        }
    }

    /// 添加流式块
    pub fn add_chunk(&mut self, chunk: StreamChunk) {
        self.total_tokens += chunk.content.len() as u32;
        if chunk.is_final {
            self.is_complete = true;
        }
        self.chunks.push(chunk);
    }

    /// 获取完整文本（拼接所有块的内容）
    pub fn full_text(&self) -> String {
        self.chunks.iter().map(|c| c.content.as_str()).collect()
    }

    /// 获取块数量
    pub fn chunk_count(&self) -> usize {
        self.chunks.len()
    }

    /// 检查会话是否已完成
    pub fn is_complete(&self) -> bool {
        self.is_complete
    }

    /// 获取最后一个块
    pub fn last_chunk(&self) -> Option<&StreamChunk> {
        self.chunks.last()
    }

    /// 获取总延迟（所有块的延迟之和）
    pub fn total_latency_ms(&self) -> u32 {
        self.chunks.iter().map(|c| c.latency_ms).sum()
    }
}

/// 流式推理管理器，管理所有活跃的流式会话
pub struct StreamManager {
    /// 活跃会话映射，键为会话 ID
    sessions: HashMap<u64, StreamSession>,
    /// 下一个会话 ID
    next_session_id: u64,
}

impl StreamManager {
    /// 创建新的流式推理管理器
    pub fn new() -> Self {
        Self {
            sessions: HashMap::new(),
            next_session_id: 1,
        }
    }

    /// 创建新的流式会话，返回会话 ID
    pub fn create_session(&mut self, model_id: ModelId) -> u64 {
        let id = self.next_session_id;
        self.next_session_id += 1;

        let session = StreamSession::new(id, model_id);
        self.sessions.insert(id, session);

        id
    }

    /// 向指定会话添加流式块
    pub fn add_chunk(
        &mut self,
        session_id: u64,
        chunk: StreamChunk,
    ) -> Result<(), InferenceError> {
        let session = self
            .sessions
            .get_mut(&session_id)
            .ok_or_else(|| InferenceError::ModelNotFound(format!("会话 {} 不存在", session_id)))?;

        if session.is_complete {
            return Err(InferenceError::InferenceFailed(format!(
                "会话 {} 已完成，无法添加新块",
                session_id
            )));
        }

        session.add_chunk(chunk);
        Ok(())
    }

    /// 获取指定会话的不可变引用
    pub fn get_session(&self, session_id: u64) -> Option<&StreamSession> {
        self.sessions.get(&session_id)
    }

    /// 完成指定会话
    pub fn complete_session(&mut self, session_id: u64) -> Result<(), InferenceError> {
        let session = self
            .sessions
            .get_mut(&session_id)
            .ok_or_else(|| InferenceError::ModelNotFound(format!("会话 {} 不存在", session_id)))?;

        session.is_complete = true;
        Ok(())
    }

    /// 取消指定会话（从管理器中移除）
    pub fn cancel_session(&mut self, session_id: u64) {
        self.sessions.remove(&session_id);
    }

    /// 获取当前活跃会话数量（未完成的会话）
    pub fn active_sessions(&self) -> usize {
        self.sessions.values().filter(|s| !s.is_complete).count()
    }

    /// 获取总会话数量（包括已完成的）
    pub fn total_sessions(&self) -> usize {
        self.sessions.len()
    }

    /// 清理已完成的会话
    pub fn cleanup_completed(&mut self) -> usize {
        let before = self.sessions.len();
        self.sessions.retain(|_, s| !s.is_complete);
        before - self.sessions.len()
    }

    /// 获取所有活跃会话的 ID 列表
    pub fn active_session_ids(&self) -> Vec<u64> {
        self.sessions
            .iter()
            .filter(|(_, s)| !s.is_complete)
            .map(|(id, _)| *id)
            .collect()
    }
}

impl Default for StreamManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stream_session_new() {
        let session = StreamSession::new(1, ModelId::new("llama-7b"));
        assert_eq!(session.id, 1);
        assert_eq!(session.model_id.as_str(), "llama-7b");
        assert_eq!(session.chunk_count(), 0);
        assert!(!session.is_complete());
        assert_eq!(session.full_text(), "");
        assert_eq!(session.total_tokens, 0);
    }

    #[test]
    fn test_stream_session_add_chunk() {
        let mut session = StreamSession::new(1, ModelId::new("llama-7b"));

        session.add_chunk(StreamChunk::new(0, "Hello ".to_string()));
        assert_eq!(session.chunk_count(), 1);
        assert_eq!(session.full_text(), "Hello ");
        assert!(!session.is_complete());

        session.add_chunk(StreamChunk::new(1, "world".to_string()));
        assert_eq!(session.chunk_count(), 2);
        assert_eq!(session.full_text(), "Hello world");
        assert!(!session.is_complete());
    }

    #[test]
    fn test_stream_session_final_chunk() {
        let mut session = StreamSession::new(1, ModelId::new("llama-7b"));

        session.add_chunk(StreamChunk::new(0, "Hello ".to_string()));
        session.add_chunk(StreamChunk::final_chunk(1, "world".to_string()));

        assert_eq!(session.chunk_count(), 2);
        assert_eq!(session.full_text(), "Hello world");
        assert!(session.is_complete());
    }

    #[test]
    fn test_stream_session_full_text_empty() {
        let session = StreamSession::new(1, ModelId::new("llama-7b"));
        assert_eq!(session.full_text(), "");
    }

    #[test]
    fn test_stream_session_full_text_multiple_chunks() {
        let mut session = StreamSession::new(1, ModelId::new("llama-7b"));

        session.add_chunk(StreamChunk::new(0, "The ".to_string()));
        session.add_chunk(StreamChunk::new(1, "quick ".to_string()));
        session.add_chunk(StreamChunk::new(2, "brown ".to_string()));
        session.add_chunk(StreamChunk::final_chunk(3, "fox".to_string()));

        assert_eq!(session.full_text(), "The quick brown fox");
    }

    #[test]
    fn test_stream_session_total_tokens() {
        let mut session = StreamSession::new(1, ModelId::new("llama-7b"));

        session.add_chunk(StreamChunk::new(0, "Hello".to_string()));
        assert_eq!(session.total_tokens, 5);

        session.add_chunk(StreamChunk::new(1, " world".to_string()));
        assert_eq!(session.total_tokens, 11);
    }

    #[test]
    fn test_stream_session_last_chunk() {
        let mut session = StreamSession::new(1, ModelId::new("llama-7b"));

        assert!(session.last_chunk().is_none());

        session.add_chunk(StreamChunk::new(0, "Hello".to_string()));
        assert_eq!(session.last_chunk().unwrap().content, "Hello");

        session.add_chunk(StreamChunk::new(1, " world".to_string()));
        assert_eq!(session.last_chunk().unwrap().content, " world");
    }

    #[test]
    fn test_stream_session_total_latency() {
        let mut session = StreamSession::new(1, ModelId::new("llama-7b"));

        session.add_chunk(StreamChunk::new(0, "Hello".to_string()).with_latency(10));
        session.add_chunk(StreamChunk::new(1, " world".to_string()).with_latency(15));

        assert_eq!(session.total_latency_ms(), 25);
    }

    #[test]
    fn test_stream_session_with_timestamp() {
        let session = StreamSession::new_with_timestamp(1, ModelId::new("llama-7b"), 12345);
        assert_eq!(session.created_at, 12345);
    }

    #[test]
    fn test_stream_manager_new() {
        let manager = StreamManager::new();
        assert_eq!(manager.active_sessions(), 0);
        assert_eq!(manager.total_sessions(), 0);
    }

    #[test]
    fn test_stream_manager_create_session() {
        let mut manager = StreamManager::new();

        let id1 = manager.create_session(ModelId::new("llama-7b"));
        assert_eq!(id1, 1);
        assert_eq!(manager.active_sessions(), 1);

        let id2 = manager.create_session(ModelId::new("gpt-4"));
        assert_eq!(id2, 2);
        assert_eq!(manager.active_sessions(), 2);
    }

    #[test]
    fn test_stream_manager_add_chunk() {
        let mut manager = StreamManager::new();
        let session_id = manager.create_session(ModelId::new("llama-7b"));

        let result = manager.add_chunk(
            session_id,
            StreamChunk::new(0, "Hello".to_string()),
        );
        assert!(result.is_ok());

        let session = manager.get_session(session_id).unwrap();
        assert_eq!(session.full_text(), "Hello");
    }

    #[test]
    fn test_stream_manager_add_chunk_nonexistent() {
        let mut manager = StreamManager::new();

        let result = manager.add_chunk(
            999,
            StreamChunk::new(0, "Hello".to_string()),
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_stream_manager_add_chunk_to_completed_session() {
        let mut manager = StreamManager::new();
        let session_id = manager.create_session(ModelId::new("llama-7b"));

        // 完成会话
        manager.complete_session(session_id).unwrap();

        // 尝试向已完成的会话添加块
        let result = manager.add_chunk(
            session_id,
            StreamChunk::new(0, "Hello".to_string()),
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_stream_manager_get_session() {
        let mut manager = StreamManager::new();
        let session_id = manager.create_session(ModelId::new("llama-7b"));

        let session = manager.get_session(session_id);
        assert!(session.is_some());
        assert_eq!(session.unwrap().model_id.as_str(), "llama-7b");

        assert!(manager.get_session(999).is_none());
    }

    #[test]
    fn test_stream_manager_complete_session() {
        let mut manager = StreamManager::new();
        let session_id = manager.create_session(ModelId::new("llama-7b"));

        manager.complete_session(session_id).unwrap();

        let session = manager.get_session(session_id).unwrap();
        assert!(session.is_complete());
        assert_eq!(manager.active_sessions(), 0);
    }

    #[test]
    fn test_stream_manager_complete_nonexistent() {
        let mut manager = StreamManager::new();
        let result = manager.complete_session(999);
        assert!(result.is_err());
    }

    #[test]
    fn test_stream_manager_cancel_session() {
        let mut manager = StreamManager::new();
        let session_id = manager.create_session(ModelId::new("llama-7b"));

        assert_eq!(manager.total_sessions(), 1);
        manager.cancel_session(session_id);
        assert_eq!(manager.total_sessions(), 0);
        assert!(manager.get_session(session_id).is_none());
    }

    #[test]
    fn test_stream_manager_cancel_nonexistent() {
        let mut manager = StreamManager::new();
        // 取消不存在的会话不应 panic
        manager.cancel_session(999);
        assert_eq!(manager.total_sessions(), 0);
    }

    #[test]
    fn test_stream_manager_active_sessions() {
        let mut manager = StreamManager::new();

        let id1 = manager.create_session(ModelId::new("llama-7b"));
        let id2 = manager.create_session(ModelId::new("gpt-4"));
        let _id3 = manager.create_session(ModelId::new("claude-3"));

        assert_eq!(manager.active_sessions(), 3);

        // 完成一个会话
        manager.complete_session(id1).unwrap();
        assert_eq!(manager.active_sessions(), 2);

        // 取消一个会话
        manager.cancel_session(id2);
        assert_eq!(manager.active_sessions(), 1);
    }

    #[test]
    fn test_stream_manager_cleanup_completed() {
        let mut manager = StreamManager::new();

        let id1 = manager.create_session(ModelId::new("llama-7b"));
        let id2 = manager.create_session(ModelId::new("gpt-4"));
        let _id3 = manager.create_session(ModelId::new("claude-3"));

        manager.complete_session(id1).unwrap();
        manager.complete_session(id2).unwrap();

        let removed = manager.cleanup_completed();
        assert_eq!(removed, 2);
        assert_eq!(manager.total_sessions(), 1);
    }

    #[test]
    fn test_stream_manager_active_session_ids() {
        let mut manager = StreamManager::new();

        let id1 = manager.create_session(ModelId::new("llama-7b"));
        let id2 = manager.create_session(ModelId::new("gpt-4"));

        manager.complete_session(id1).unwrap();

        let active_ids = manager.active_session_ids();
        assert_eq!(active_ids.len(), 1);
        assert!(active_ids.contains(&id2));
        assert!(!active_ids.contains(&id1));
    }

    #[test]
    fn test_stream_manager_full_workflow() {
        let mut manager = StreamManager::new();

        // 创建会话
        let session_id = manager.create_session(ModelId::new("llama-7b"));
        assert_eq!(manager.active_sessions(), 1);

        // 添加多个块
        manager
            .add_chunk(session_id, StreamChunk::new(0, "Hello ".to_string()).with_latency(5))
            .unwrap();
        manager
            .add_chunk(session_id, StreamChunk::new(1, "from ".to_string()).with_latency(3))
            .unwrap();
        manager
            .add_chunk(
                session_id,
                StreamChunk::final_chunk(2, "LLaMA".to_string()).with_latency(8),
            )
            .unwrap();

        // 验证结果
        let session = manager.get_session(session_id).unwrap();
        assert_eq!(session.full_text(), "Hello from LLaMA");
        assert_eq!(session.chunk_count(), 3);
        assert!(session.is_complete());
        assert_eq!(session.total_latency_ms(), 16);
        assert_eq!(manager.active_sessions(), 0);
    }

    #[test]
    fn test_stream_manager_default() {
        let manager = StreamManager::default();
        assert_eq!(manager.active_sessions(), 0);
    }
}
