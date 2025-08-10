#!/bin/bash

# Licensed under the Apache License, Version 2.0 (the "License");
# you may not use this file except in compliance with the License.
# You may obtain a copy of the License at
#
#     http://www.apache.org/licenses/LICENSE-2.0
#
# Unless required by applicable law or agreed to in writing, software
# distributed under the License is distributed on an "AS IS" BASIS,
# WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
# See the License for the specific language governing permissions and
# limitations under the License.

set -e  # Exit on any error

# Ensure the script is run from the project root
cargo build --features "git sql" --bin prolly-ui

# Get project root path dynamically
if [[ ! -f "Cargo.toml" ]]; then
    echo "❌ Error: Run this script from the ProllyTree project root directory"
    exit 1
fi

PROJECT_ROOT=$(pwd)
DEMO_DIR="/tmp/ai-agent-demo"
PROLLY_BIN="$PROJECT_ROOT/target/debug/git-prolly"
UI_BIN="$PROJECT_ROOT/target/debug/prolly-ui"

echo "🤖 Setting up AI Agent multi-dataset demo..."
echo "📁 Demo directory: $DEMO_DIR"
echo "🔧 Project root: $PROJECT_ROOT"

# Clean up and create fresh demo directory
rm -rf "$DEMO_DIR"
mkdir -p "$DEMO_DIR"

cd "$DEMO_DIR"

# Initialize git repository first
git init

# Configure git for the dataset's individual repository
git config user.name "AI Agent System"
git config user.email "agent@ai-system.com"

# Create datasets for AI agent system
DATASETS=("prompts" "memory_short" "memory_long" "conversations" "sessions" "context" "embeddings" "models" "agents")

for dataset in "${DATASETS[@]}"; do
    echo ""
    echo "🤖 Creating AI dataset: $dataset"

    # Create dataset directory structure
    mkdir -p "$dataset"
    cd "$dataset"

    # Initialize git-prolly in the data subdirectory
    "$PROLLY_BIN" init

    cd "$DEMO_DIR"
done

echo ""
echo "🔄 Populating AI agent datasets..."

# Dataset 1: Prompts - System prompts, user prompts, templates
echo "💬 Populating prompts dataset..."

# Main branch - System prompts
(cd "$DEMO_DIR/prompts" && "$PROLLY_BIN" set "system:assistant" "You are a helpful AI assistant. Be concise, accurate, and helpful in your responses.")
(cd "$DEMO_DIR/prompts" && "$PROLLY_BIN" set "system:code_reviewer" "You are an expert code reviewer. Analyze code for bugs, security issues, and best practices.")
(cd "$DEMO_DIR/prompts" && "$PROLLY_BIN" set "system:creative_writer" "You are a creative writing assistant. Help with storytelling, character development, and narrative structure.")
(cd "$DEMO_DIR/prompts" && "$PROLLY_BIN" commit -m "Initial system prompts")

(cd "$DEMO_DIR/prompts" && "$PROLLY_BIN" set "template:email_draft" "Subject: {subject}\n\nDear {recipient},\n\n{body}\n\nBest regards,\n{sender}")
(cd "$DEMO_DIR/prompts" && "$PROLLY_BIN" set "template:code_explanation" "This code does the following:\n1. {step1}\n2. {step2}\n3. {step3}\n\nKey concepts: {concepts}")
(cd "$DEMO_DIR/prompts" && "$PROLLY_BIN" commit -m "Add prompt templates")

# Create specialized prompt branch
(cd "$DEMO_DIR/prompts" && git checkout -b prompt-engineering/optimization)
(cd "$DEMO_DIR/prompts" && "$PROLLY_BIN" set "system:assistant" "You are a helpful AI assistant. Be concise, accurate, and helpful. Always ask clarifying questions when context is unclear.")
(cd "$DEMO_DIR/prompts" && "$PROLLY_BIN" set "system:research_assistant" "You are a research assistant specialized in finding accurate information and citing sources.")
(cd "$DEMO_DIR/prompts" && "$PROLLY_BIN" commit -m "Optimize system prompts for clarity")

(cd "$DEMO_DIR/prompts" && "$PROLLY_BIN" set "template:meeting_summary" "Meeting: {title}\nDate: {date}\nAttendees: {attendees}\n\nKey Points:\n{key_points}\n\nAction Items:\n{action_items}")
(cd "$DEMO_DIR/prompts" && "$PROLLY_BIN" set "few_shot:sentiment" "Examples:\nInput: 'I love this product!' -> Positive\nInput: 'This is terrible' -> Negative\nInput: 'It's okay I guess' -> Neutral")
(cd "$DEMO_DIR/prompts" && "$PROLLY_BIN" commit -m "Add advanced prompt templates and few-shot examples")

# Create domain-specific prompts branch
(cd "$DEMO_DIR/prompts" && git checkout -b domain/specialized)
(cd "$DEMO_DIR/prompts" && "$PROLLY_BIN" set "domain:medical" "You are a medical information assistant. Provide accurate health information but always recommend consulting healthcare professionals.")
(cd "$DEMO_DIR/prompts" && "$PROLLY_BIN" set "domain:legal" "You are a legal research assistant. Provide general legal information but always recommend consulting qualified attorneys.")
(cd "$DEMO_DIR/prompts" && "$PROLLY_BIN" set "domain:financial" "You are a financial analysis assistant. Provide market insights but not investment advice.")
(cd "$DEMO_DIR/prompts" && "$PROLLY_BIN" commit -m "Add domain-specific prompt configurations")

# Back to main
(cd "$DEMO_DIR/prompts" && git checkout main)

# Dataset 2: Short-term Memory - Recent context, active conversations
echo "🧠 Populating short-term memory dataset..."

# Main branch - Active session memory
(cd "$DEMO_DIR/memory_short" && "$PROLLY_BIN" set "session:user123_active" "last_topic:code_review|context_window:4096|active_since:2024-08-10T09:30:00Z")
(cd "$DEMO_DIR/memory_short" && "$PROLLY_BIN" set "session:user456_active" "last_topic:creative_writing|context_window:2048|active_since:2024-08-10T10:15:00Z")
(cd "$DEMO_DIR/memory_short" && "$PROLLY_BIN" set "working_memory:current_task" "analyzing_python_code|file:main.py|line_focus:45-67|issue:potential_memory_leak")
(cd "$DEMO_DIR/memory_short" && "$PROLLY_BIN" commit -m "Initialize active session memory")

(cd "$DEMO_DIR/memory_short" && "$PROLLY_BIN" set "context:recent_files" "files:[main.py,utils.py,config.json]|last_modified:2024-08-10T09:45:00Z")
(cd "$DEMO_DIR/memory_short" && "$PROLLY_BIN" set "context:user_preferences" "user123:verbose_explanations|code_style:pythonic|preferred_language:python")
(cd "$DEMO_DIR/memory_short" && "$PROLLY_BIN" commit -m "Add contextual memory elements")

# Create memory management branch
(cd "$DEMO_DIR/memory_short" && git checkout -b memory/cleanup)
(cd "$DEMO_DIR/memory_short" && "$PROLLY_BIN" set "session:user789_expired" "last_topic:data_analysis|context_window:2048|expired:2024-08-10T08:30:00Z|ttl:3600")
(cd "$DEMO_DIR/memory_short" && "$PROLLY_BIN" set "cleanup:stale_sessions" "count:5|last_cleanup:2024-08-10T08:00:00Z|next_cleanup:2024-08-10T12:00:00Z")
(cd "$DEMO_DIR/memory_short" && "$PROLLY_BIN" commit -m "Track expired sessions for cleanup")

(cd "$DEMO_DIR/memory_short" && "$PROLLY_BIN" set "working_memory:current_task" "writing_story|genre:sci-fi|character_focus:protagonist|scene:space_station")
(cd "$DEMO_DIR/memory_short" && "$PROLLY_BIN" set "context:conversation_flow" "turn_count:15|last_user_intent:clarification|agent_mode:creative")
(cd "$DEMO_DIR/memory_short" && "$PROLLY_BIN" commit -m "Update active working memory")

# Create attention mechanism branch
(cd "$DEMO_DIR/memory_short" && git checkout -b attention/focus)
(cd "$DEMO_DIR/memory_short" && "$PROLLY_BIN" set "attention:high_priority" "items:[bug_fix_request,deadline_tomorrow,critical_security_issue]")
(cd "$DEMO_DIR/memory_short" && "$PROLLY_BIN" set "attention:context_relevance" "current_score:0.85|threshold:0.7|auto_prune:enabled")
(cd "$DEMO_DIR/memory_short" && "$PROLLY_BIN" commit -m "Implement attention-based memory prioritization")

# Back to main
(cd "$DEMO_DIR/memory_short" && git checkout main)

# Dataset 3: Long-term Memory - Persistent knowledge, learned patterns
echo "🗄️ Populating long-term memory dataset..."

# Main branch - User profiles and learned patterns
(cd "$DEMO_DIR/memory_long" && "$PROLLY_BIN" set "user_profile:123" "name:Alice|expertise:python,machine_learning|communication_style:technical|session_count:47")
(cd "$DEMO_DIR/memory_long" && "$PROLLY_BIN" set "user_profile:456" "name:Bob|expertise:creative_writing,storytelling|communication_style:collaborative|session_count:23")
(cd "$DEMO_DIR/memory_long" && "$PROLLY_BIN" set "learned_pattern:code_review" "common_issues:[memory_leaks,sql_injection,error_handling]|success_rate:0.87")
(cd "$DEMO_DIR/memory_long" && "$PROLLY_BIN" commit -m "Initialize user profiles and learned patterns")

(cd "$DEMO_DIR/memory_long" && "$PROLLY_BIN" set "knowledge_base:python_best_practices" "last_updated:2024-08-01|version:1.2|confidence:0.92|source:code_reviews")
(cd "$DEMO_DIR/memory_long" && "$PROLLY_BIN" set "knowledge_base:creative_techniques" "storytelling_methods:[three_act,heros_journey,freytag]|effectiveness:0.78")
(cd "$DEMO_DIR/memory_long" && "$PROLLY_BIN" commit -m "Build knowledge base entries")

# Create personalization branch
(cd "$DEMO_DIR/memory_long" && git checkout -b personalization/adaptive)
(cd "$DEMO_DIR/memory_long" && "$PROLLY_BIN" set "user_profile:123" "name:Alice|expertise:python,machine_learning,data_science|communication_style:technical|session_count:52|preferred_detail:high")
(cd "$DEMO_DIR/memory_long" && "$PROLLY_BIN" set "adaptation:user123" "learning_rate:0.1|feedback_positive:0.89|preferred_response_length:detailed")
(cd "$DEMO_DIR/memory_long" && "$PROLLY_BIN" commit -m "Enhance user profiles with adaptation metrics")

(cd "$DEMO_DIR/memory_long" && "$PROLLY_BIN" set "behavioral_pattern:user123" "typical_session_duration:45min|peak_activity:10am-2pm|common_requests:[code_review,debugging,optimization]")
(cd "$DEMO_DIR/memory_long" && "$PROLLY_BIN" set "behavioral_pattern:user456" "typical_session_duration:30min|peak_activity:7pm-10pm|common_requests:[story_brainstorming,character_development]")
(cd "$DEMO_DIR/memory_long" && "$PROLLY_BIN" commit -m "Track behavioral patterns for personalization")

# Create knowledge graph branch
(cd "$DEMO_DIR/memory_long" && git checkout -b knowledge/graph)
(cd "$DEMO_DIR/memory_long" && "$PROLLY_BIN" set "concept:machine_learning" "related:[python,data_science,algorithms]|strength:0.95|last_reinforced:2024-08-09")
(cd "$DEMO_DIR/memory_long" && "$PROLLY_BIN" set "concept:creative_writing" "related:[storytelling,character_development,plot_structure]|strength:0.87|applications:23")
(cd "$DEMO_DIR/memory_long" && "$PROLLY_BIN" set "connection:python_ml" "from:python|to:machine_learning|weight:0.92|co_occurrence:156")
(cd "$DEMO_DIR/memory_long" && "$PROLLY_BIN" commit -m "Build semantic knowledge graph connections")

# Back to main
(cd "$DEMO_DIR/memory_long" && git checkout main)

# Dataset 4: Conversations - Chat history, dialogue management
echo "💭 Populating conversations dataset..."

# Main branch - Recent conversations
(cd "$DEMO_DIR/conversations" && "$PROLLY_BIN" set "conv:20240810_123_001" "user:123|start:09:30:00|end:10:15:00|turns:12|topic:python_debugging|satisfaction:high")
(cd "$DEMO_DIR/conversations" && "$PROLLY_BIN" set "conv:20240810_456_001" "user:456|start:10:00:00|end:10:45:00|turns:18|topic:story_development|satisfaction:high")
(cd "$DEMO_DIR/conversations" && "$PROLLY_BIN" set "conv:20240810_789_001" "user:789|start:11:30:00|end:12:00:00|turns:8|topic:data_analysis|satisfaction:medium")
(cd "$DEMO_DIR/conversations" && "$PROLLY_BIN" commit -m "Log completed conversations")

(cd "$DEMO_DIR/conversations" && "$PROLLY_BIN" set "turn:20240810_123_001_t01" "speaker:user|content:Can you help me debug this Python function?|timestamp:09:30:15|intent:help_request")
(cd "$DEMO_DIR/conversations" && "$PROLLY_BIN" set "turn:20240810_123_001_t02" "speaker:assistant|content:I'd be happy to help debug your Python function...|timestamp:09:30:18|tokens:156")
(cd "$DEMO_DIR/conversations" && "$PROLLY_BIN" set "turn:20240810_123_001_t03" "speaker:user|content:Here's the function: def calculate_average()...|timestamp:09:30:45|code_included:true")
(cd "$DEMO_DIR/conversations" && "$PROLLY_BIN" commit -m "Store conversation turns and dialogue history")

# Create conversation analysis branch
(cd "$DEMO_DIR/conversations" && git checkout -b analysis/patterns)
(cd "$DEMO_DIR/conversations" && "$PROLLY_BIN" set "pattern:debug_sessions" "avg_turns:14.2|success_rate:0.89|common_issues:[syntax_error,logic_error,performance]")
(cd "$DEMO_DIR/conversations" && "$PROLLY_BIN" set "pattern:creative_sessions" "avg_turns:22.5|satisfaction:0.92|common_requests:[brainstorming,character_creation,plot_development]")
(cd "$DEMO_DIR/conversations" && "$PROLLY_BIN" commit -m "Analyze conversation patterns")

(cd "$DEMO_DIR/conversations" && "$PROLLY_BIN" set "metric:user_engagement" "avg_session_length:38min|return_rate:0.78|satisfaction_score:4.2/5")
(cd "$DEMO_DIR/conversations" && "$PROLLY_BIN" set "metric:topic_distribution" "technical:45%|creative:30%|general:25%|trend:increasing_technical")
(cd "$DEMO_DIR/conversations" && "$PROLLY_BIN" commit -m "Track engagement and topic metrics")

# Create dialogue quality branch
(cd "$DEMO_DIR/conversations" && git checkout -b quality/assessment)
(cd "$DEMO_DIR/conversations" && "$PROLLY_BIN" set "quality:helpfulness" "score:4.3/5|feedback_count:127|improvement_trend:positive")
(cd "$DEMO_DIR/conversations" && "$PROLLY_BIN" set "quality:coherence" "score:4.1/5|context_maintenance:0.87|topic_drift_rate:0.08")
(cd "$DEMO_DIR/conversations" && "$PROLLY_BIN" set "quality:accuracy" "technical_accuracy:0.91|fact_checking_score:0.88|correction_rate:0.05")
(cd "$DEMO_DIR/conversations" && "$PROLLY_BIN" commit -m "Assess dialogue quality metrics")

# Back to main
(cd "$DEMO_DIR/conversations" && git checkout main)

# Dataset 5: Sessions - User sessions, authentication, state management
echo "🔐 Populating sessions dataset..."

# Main branch - Active sessions
(cd "$DEMO_DIR/sessions" && "$PROLLY_BIN" set "session:sess_789abc123" "user_id:123|start:2024-08-10T09:30:00Z|last_activity:2024-08-10T10:45:00Z|status:active")
(cd "$DEMO_DIR/sessions" && "$PROLLY_BIN" set "session:sess_def456789" "user_id:456|start:2024-08-10T10:00:00Z|last_activity:2024-08-10T11:30:00Z|status:active")
(cd "$DEMO_DIR/sessions" && "$PROLLY_BIN" set "session:sess_ghi123456" "user_id:789|start:2024-08-10T08:15:00Z|last_activity:2024-08-10T12:00:00Z|status:expired")
(cd "$DEMO_DIR/sessions" && "$PROLLY_BIN" commit -m "Track user sessions")

(cd "$DEMO_DIR/sessions" && "$PROLLY_BIN" set "auth:user_123" "login_time:2024-08-10T09:29:45Z|auth_method:oauth|permissions:[read,write,admin]|mfa_enabled:true")
(cd "$DEMO_DIR/sessions" && "$PROLLY_BIN" set "auth:user_456" "login_time:2024-08-10T09:59:30Z|auth_method:password|permissions:[read,write]|mfa_enabled:false")
(cd "$DEMO_DIR/sessions" && "$PROLLY_BIN" commit -m "Store authentication details")

# Create session management branch
(cd "$DEMO_DIR/sessions" && git checkout -b management/lifecycle)
(cd "$DEMO_DIR/sessions" && "$PROLLY_BIN" set "session:sess_789abc123" "user_id:123|start:2024-08-10T09:30:00Z|last_activity:2024-08-10T11:15:00Z|status:active|requests:47")
(cd "$DEMO_DIR/sessions" && "$PROLLY_BIN" set "lifecycle:cleanup_policy" "max_idle:30min|max_duration:4hours|cleanup_interval:15min|retention:7days")
(cd "$DEMO_DIR/sessions" && "$PROLLY_BIN" commit -m "Update session lifecycle management")

(cd "$DEMO_DIR/sessions" && "$PROLLY_BIN" set "security:anomaly_detection" "suspicious_login_attempts:3|blocked_ips:[192.168.1.100,10.0.0.50]|last_scan:2024-08-10T11:00:00Z")
(cd "$DEMO_DIR/sessions" && "$PROLLY_BIN" set "security:rate_limiting" "user_123:requests_per_hour:120|user_456:requests_per_hour:85|global_limit:1000")
(cd "$DEMO_DIR/sessions" && "$PROLLY_BIN" commit -m "Implement security monitoring")

# Create analytics branch
(cd "$DEMO_DIR/sessions" && git checkout -b analytics/usage)
(cd "$DEMO_DIR/sessions" && "$PROLLY_BIN" set "usage:daily_active_users" "date:2024-08-10|count:247|peak_hour:2pm|avg_session_duration:42min")
(cd "$DEMO_DIR/sessions" && "$PROLLY_BIN" set "usage:feature_adoption" "code_review:78%|creative_writing:45%|general_chat:92%|new_features:23%")
(cd "$DEMO_DIR/sessions" && "$PROLLY_BIN" commit -m "Track usage analytics")

# Back to main
(cd "$DEMO_DIR/sessions" && git checkout main)

# Dataset 6: Context - Environmental context, task context, domain context
echo "📋 Populating context dataset..."

# Main branch - Current context
(cd "$DEMO_DIR/context" && "$PROLLY_BIN" set "env:development" "mode:debug|log_level:info|features:[experimental_ui,beta_models]|version:v2.1.0-beta")
(cd "$DEMO_DIR/context" && "$PROLLY_BIN" set "env:user_123_workspace" "project:python_ml_pipeline|files_open:5|current_branch:feature/optimization")
(cd "$DEMO_DIR/context" && "$PROLLY_BIN" set "task:active" "type:code_review|priority:high|deadline:2024-08-10T18:00:00Z|progress:0.6")
(cd "$DEMO_DIR/context" && "$PROLLY_BIN" commit -m "Initialize environmental context")

(cd "$DEMO_DIR/context" && "$PROLLY_BIN" set "domain:technical" "expertise_level:advanced|preferred_depth:detailed|include_examples:true|language:python")
(cd "$DEMO_DIR/context" && "$PROLLY_BIN" set "domain:creative" "genre_preference:sci-fi|tone:collaborative|creativity_level:high|story_structure:three_act")
(cd "$DEMO_DIR/context" && "$PROLLY_BIN" commit -m "Set domain-specific context")

# Create context switching branch
(cd "$DEMO_DIR/context" && git checkout -b context/switching)
(cd "$DEMO_DIR/context" && "$PROLLY_BIN" set "switch:previous_context" "domain:technical|task:debugging|saved_state:line_45_analysis|timestamp:2024-08-10T10:30:00Z")
(cd "$DEMO_DIR/context" && "$PROLLY_BIN" set "switch:current_context" "domain:creative|task:story_development|character:protagonist|scene:chapter_3")
(cd "$DEMO_DIR/context" && "$PROLLY_BIN" commit -m "Handle context switching between domains")

(cd "$DEMO_DIR/context" && "$PROLLY_BIN" set "relevance:technical_context" "relevance_score:0.92|decay_rate:0.1|last_reinforcement:2024-08-10T11:00:00Z")
(cd "$DEMO_DIR/context" && "$PROLLY_BIN" set "relevance:creative_context" "relevance_score:0.78|active_elements:[character,setting,plot]|narrative_position:act_2")
(cd "$DEMO_DIR/context" && "$PROLLY_BIN" commit -m "Track context relevance and decay")

# Create context integration branch
(cd "$DEMO_DIR/context" && git checkout -b integration/multi-modal)
(cd "$DEMO_DIR/context" && "$PROLLY_BIN" set "multimodal:code_context" "file_type:python|syntax_highlighting:true|imports:[numpy,pandas,sklearn]|line_count:234")
(cd "$DEMO_DIR/context" && "$PROLLY_BIN" set "multimodal:visual_context" "diagrams_present:true|chart_types:[line_plot,scatter,histogram]|complexity:medium")
(cd "$DEMO_DIR/context" && "$PROLLY_BIN" set "multimodal:document_context" "format:markdown|headers:5|code_blocks:8|word_count:1250")
(cd "$DEMO_DIR/context" && "$PROLLY_BIN" commit -m "Integrate multi-modal context understanding")

# Back to main
(cd "$DEMO_DIR/context" && git checkout main)

# Dataset 7: Embeddings - Vector embeddings, semantic search, similarity
echo "🧮 Populating embeddings dataset..."

# Main branch - Core embeddings
(cd "$DEMO_DIR/embeddings" && "$PROLLY_BIN" set "embed:doc_001" "text:python debugging techniques|vector_id:v_789abc|model:text-embedding-3-small|dimension:1536")
(cd "$DEMO_DIR/embeddings" && "$PROLLY_BIN" set "embed:doc_002" "text:creative writing story structure|vector_id:v_def456|model:text-embedding-3-small|dimension:1536")
(cd "$DEMO_DIR/embeddings" && "$PROLLY_BIN" set "embed:doc_003" "text:machine learning pipeline optimization|vector_id:v_ghi789|model:text-embedding-3-small|dimension:1536")
(cd "$DEMO_DIR/embeddings" && "$PROLLY_BIN" commit -m "Store document embeddings")

(cd "$DEMO_DIR/embeddings" && "$PROLLY_BIN" set "similarity:cluster_01" "documents:[doc_001,doc_003]|similarity_score:0.87|topic:technical_programming")
(cd "$DEMO_DIR/embeddings" && "$PROLLY_BIN" set "similarity:cluster_02" "documents:[doc_002]|similarity_score:1.0|topic:creative_writing")
(cd "$DEMO_DIR/embeddings" && "$PROLLY_BIN" commit -m "Identify similarity clusters")

# Create semantic search branch
(cd "$DEMO_DIR/embeddings" && git checkout -b semantic/search)
(cd "$DEMO_DIR/embeddings" && "$PROLLY_BIN" set "search_index:technical" "documents:156|last_updated:2024-08-10T10:00:00Z|avg_relevance:0.84|query_count:47")
(cd "$DEMO_DIR/embeddings" && "$PROLLY_BIN" set "search_index:creative" "documents:89|last_updated:2024-08-10T09:45:00Z|avg_relevance:0.79|query_count:23")
(cd "$DEMO_DIR/embeddings" && "$PROLLY_BIN" commit -m "Build semantic search indices")

(cd "$DEMO_DIR/embeddings" && "$PROLLY_BIN" set "query:recent_001" "text:how to optimize python code|results:5|top_score:0.91|response_time:120ms")
(cd "$DEMO_DIR/embeddings" && "$PROLLY_BIN" set "query:recent_002" "text:character development techniques|results:3|top_score:0.88|response_time:95ms")
(cd "$DEMO_DIR/embeddings" && "$PROLLY_BIN" commit -m "Log semantic search queries")

# Create vector optimization branch
(cd "$DEMO_DIR/embeddings" && git checkout -b optimization/vectors)
(cd "$DEMO_DIR/embeddings" && "$PROLLY_BIN" set "optimization:dimensionality" "original:1536|reduced:512|method:pca|variance_retained:0.95")
(cd "$DEMO_DIR/embeddings" && "$PROLLY_BIN" set "optimization:quantization" "precision:float32|compressed:int8|compression_ratio:4x|accuracy_loss:0.02")
(cd "$DEMO_DIR/embeddings" && "$PROLLY_BIN" set "performance:search_speed" "avg_query_time:85ms|index_size:2.3GB|memory_usage:1.8GB")
(cd "$DEMO_DIR/embeddings" && "$PROLLY_BIN" commit -m "Optimize vector storage and retrieval")

# Back to main
(cd "$DEMO_DIR/embeddings" && git checkout main)

# Dataset 8: Models - AI model configurations, performance, versions
echo "⚙️ Populating models dataset..."

# Main branch - Model configurations
(cd "$DEMO_DIR/models" && "$PROLLY_BIN" set "model:gpt-4" "version:gpt-4-0125-preview|context_window:128000|cost_per_token:0.00003|status:active")
(cd "$DEMO_DIR/models" && "$PROLLY_BIN" set "model:claude-3" "version:claude-3-sonnet|context_window:200000|cost_per_token:0.000015|status:active")
(cd "$DEMO_DIR/models" && "$PROLLY_BIN" set "model:embedding" "version:text-embedding-3-small|dimension:1536|cost_per_token:0.00000002|status:active")
(cd "$DEMO_DIR/models" && "$PROLLY_BIN" commit -m "Configure AI model settings")

(cd "$DEMO_DIR/models" && "$PROLLY_BIN" set "performance:gpt-4" "avg_response_time:2.3s|success_rate:0.98|user_satisfaction:4.4/5|daily_requests:1247")
(cd "$DEMO_DIR/models" && "$PROLLY_BIN" set "performance:claude-3" "avg_response_time:1.8s|success_rate:0.97|user_satisfaction:4.3/5|daily_requests:892")
(cd "$DEMO_DIR/models" && "$PROLLY_BIN" commit -m "Track model performance metrics")

# Create model comparison branch
(cd "$DEMO_DIR/models" && git checkout -b evaluation/comparison)
(cd "$DEMO_DIR/models" && "$PROLLY_BIN" set "benchmark:code_tasks" "gpt-4_score:0.89|claude-3_score:0.87|test_cases:150|criteria:correctness,efficiency")
(cd "$DEMO_DIR/models" && "$PROLLY_BIN" set "benchmark:creative_tasks" "gpt-4_score:0.82|claude-3_score:0.85|test_cases:75|criteria:creativity,coherence")
(cd "$DEMO_DIR/models" && "$PROLLY_BIN" set "benchmark:reasoning_tasks" "gpt-4_score:0.91|claude-3_score:0.88|test_cases:200|criteria:logic,accuracy")
(cd "$DEMO_DIR/models" && "$PROLLY_BIN" commit -m "Benchmark model performance across task types")

(cd "$DEMO_DIR/models" && "$PROLLY_BIN" set "cost_analysis:monthly" "gpt-4_cost:$1247.50|claude-3_cost:$892.30|embedding_cost:$15.75|total:$2155.55")
(cd "$DEMO_DIR/models" && "$PROLLY_BIN" set "usage_optimization" "peak_hours:[10am-2pm,7pm-9pm]|model_routing:cost_optimized|fallback_enabled:true")
(cd "$DEMO_DIR/models" && "$PROLLY_BIN" commit -m "Analyze costs and optimize usage patterns")

# Create fine-tuning branch
(cd "$DEMO_DIR/models" && git checkout -b fine-tuning/custom)
(cd "$DEMO_DIR/models" && "$PROLLY_BIN" set "fine_tune:code_review_v1" "base_model:gpt-4|training_samples:500|validation_accuracy:0.94|status:deployed")
(cd "$DEMO_DIR/models" && "$PROLLY_BIN" set "fine_tune:creative_writing_v1" "base_model:claude-3|training_samples:300|validation_accuracy:0.89|status:testing")
(cd "$DEMO_DIR/models" && "$PROLLY_BIN" set "training_data:code_review" "samples:500|quality_score:0.92|last_updated:2024-08-05|source:user_interactions")
(cd "$DEMO_DIR/models" && "$PROLLY_BIN" commit -m "Track fine-tuning experiments")

# Back to main
(cd "$DEMO_DIR/models" && git checkout main)

# Dataset 9: Agents - Agent configurations, capabilities, workflows
echo "🤖 Populating agents dataset..."

# Main branch - Agent definitions
(cd "$DEMO_DIR/agents" && "$PROLLY_BIN" set "agent:code_reviewer" "type:specialized|capabilities:[bug_detection,security_analysis,performance_review]|model:gpt-4")
(cd "$DEMO_DIR/agents" && "$PROLLY_BIN" set "agent:creative_writer" "type:specialized|capabilities:[story_development,character_creation,dialogue_writing]|model:claude-3")
(cd "$DEMO_DIR/agents" && "$PROLLY_BIN" set "agent:research_assistant" "type:general|capabilities:[information_gathering,fact_checking,source_citation]|model:gpt-4")
(cd "$DEMO_DIR/agents" && "$PROLLY_BIN" commit -m "Define specialized agent configurations")

(cd "$DEMO_DIR/agents" && "$PROLLY_BIN" set "workflow:code_review_pipeline" "stages:[initial_scan,deep_analysis,recommendation_generation]|avg_time:5.2min|success_rate:0.91")
(cd "$DEMO_DIR/agents" && "$PROLLY_BIN" set "workflow:story_creation" "stages:[concept_development,character_creation,plot_outline,scene_writing]|avg_time:12.8min|completion_rate:0.87")
(cd "$DEMO_DIR/agents" && "$PROLLY_BIN" commit -m "Configure agent workflows")

# Create agent orchestration branch
(cd "$DEMO_DIR/agents" && git checkout -b orchestration/multi-agent)
(cd "$DEMO_DIR/agents" && "$PROLLY_BIN" set "collaboration:code_review_team" "agents:[code_reviewer,security_specialist,performance_optimizer]|coordination:sequential")
(cd "$DEMO_DIR/agents" && "$PROLLY_BIN" set "collaboration:creative_team" "agents:[creative_writer,editor,researcher]|coordination:collaborative")
(cd "$DEMO_DIR/agents" && "$PROLLY_BIN" commit -m "Implement multi-agent collaboration")

(cd "$DEMO_DIR/agents" && "$PROLLY_BIN" set "routing:task_assignment" "code_tasks:code_reviewer|creative_tasks:creative_writer|research_tasks:research_assistant")
(cd "$DEMO_DIR/agents" && "$PROLLY_BIN" set "routing:load_balancing" "code_reviewer:active_tasks:3|creative_writer:active_tasks:1|research_assistant:active_tasks:2")
(cd "$DEMO_DIR/agents" && "$PROLLY_BIN" commit -m "Implement intelligent task routing")

# Create learning and adaptation branch
(cd "$DEMO_DIR/agents" && git checkout -b learning/adaptation)
(cd "$DEMO_DIR/agents" && "$PROLLY_BIN" set "learning:code_reviewer" "feedback_score:4.2/5|improvement_rate:0.05/month|successful_reviews:247|failed_reviews:12")
(cd "$DEMO_DIR/agents" && "$PROLLY_BIN" set "learning:creative_writer" "creativity_score:4.0/5|user_satisfaction:0.87|stories_completed:45|iterations_avg:2.3")
(cd "$DEMO_DIR/agents" && "$PROLLY_BIN" set "adaptation:preferences" "user_123:prefers_detailed_explanations|user_456:prefers_collaborative_approach")
(cd "$DEMO_DIR/agents" && "$PROLLY_BIN" commit -m "Track learning progress and user adaptations")

(cd "$DEMO_DIR/agents" && "$PROLLY_BIN" set "capability:emergent_behaviors" "cross_domain_knowledge:0.73|creative_problem_solving:0.68|contextual_adaptation:0.81")
(cd "$DEMO_DIR/agents" && "$PROLLY_BIN" set "capability:specialization_depth" "code_review:expert|creative_writing:advanced|research:intermediate|general_chat:expert")
(cd "$DEMO_DIR/agents" && "$PROLLY_BIN" commit -m "Assess emergent capabilities and specialization levels")

# Back to main
(cd "$DEMO_DIR/agents" && git checkout main)

## Generate the HTML output in a temporary directory
echo "📊 Generating AI agent multi-dataset visualization..."

# Create temporary directory for HTML output
TEMP_DIR=$(mktemp -d)
HTML_OUTPUT="$TEMP_DIR/ai-agent-ui.html"

"$UI_BIN" "$DEMO_DIR" -o "$HTML_OUTPUT"

echo ""
echo "✅ AI Agent visualization generated successfully!"
echo "  📄 Output file: $HTML_OUTPUT"
echo "  🤖 AI Agent multi-dataset view with comprehensive agent system data"
echo "  📊 Features: 9 AI-focused datasets with specialized branches and workflows"

cd "$PROJECT_ROOT"

# Open the HTML file from temp directory
echo "🌐 Opening AI agent visualization in browser..."
open "$HTML_OUTPUT"
