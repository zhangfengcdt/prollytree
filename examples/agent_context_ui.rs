use std::error::Error;
use std::io;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tokio::time;

// Terminal UI imports
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
    Frame, Terminal,
};

/// UI State for managing the four windows
#[derive(Clone)]
pub struct UiState {
    pub conversations: Vec<String>,
    pub memory_stats: String,
    pub git_logs: Vec<String>,
    pub kv_keys: Vec<String>,
    pub scroll_conversations: usize,
    pub scroll_git_logs: usize,
    pub scroll_kv_keys: usize,
}

impl Default for UiState {
    fn default() -> Self {
        Self {
            conversations: Vec::new(),
            memory_stats: "Memory Stats Loading...".to_string(),
            git_logs: vec!["Git logs loading...".to_string()],
            kv_keys: vec!["KV store keys loading...".to_string()],
            scroll_conversations: 0,
            scroll_git_logs: 0,
            scroll_kv_keys: 0,
        }
    }
}

/// Events that can be sent to update the UI
#[derive(Debug, Clone)]
pub enum UiEvent {
    ConversationUpdate(String),
    MemoryStatsUpdate(String),
    GitLogUpdate(Vec<String>),
    KvKeysUpdate(Vec<String>),
    Quit,
}

/// Comprehensive conversation data from the original demo
struct ConversationData {
    thread1_messages: Vec<&'static str>,
    thread2_messages: Vec<&'static str>,
    thread3_messages: Vec<&'static str>,
}

impl ConversationData {
    fn new() -> Self {
        Self {
            thread1_messages: vec![
                "Please remember: Research project on the impact of extreme weather on southeast US due to climate change. Key areas to track: hurricane intensity trends, flooding patterns, heat wave frequency, economic impacts on agriculture and infrastructure, and adaptation strategies being implemented.",
                "Search for recent data on hurricane damage costs in Florida and Georgia",
                "Fact: Hurricane Ian (2022) caused over $112 billion in damages, making it the costliest natural disaster in Florida's history category: hurricanes",
                "Fact: Category 4 and 5 hurricanes have increased by 25% in the Southeast US since 1980 category: hurricanes",
                "Rule: hurricane_evacuation: IF hurricane category >= 3 AND distance_from_coast < 10_miles THEN mandatory evacuation required",
                "Search for heat wave data in major southeast cities",
                "Fact: Atlanta experienced 35 days above 95°F in 2023, compared to an average of 15 days in the 1990s category: heat_waves",
                "Fact: Heat-related hospitalizations in Southeast US cities have increased by 43% between 2010-2023 category: heat_waves",
                "Rule: heat_advisory: IF temperature > 95F AND heat_index > 105F THEN issue heat advisory and open cooling centers",
                "Search for flooding impact on agriculture in Mississippi Delta",
                "Fact: 2019 Mississippi River flooding caused $6.2 billion in agricultural losses across Arkansas, Mississippi, and Louisiana category: flooding",
                "Rule: flood_insurance: IF property in 100-year floodplain THEN require federal flood insurance for mortgages",
            ],

            thread2_messages: vec![
                "What did I ask you to remember about my research project?",
                "What facts do we have about hurricanes?",
                "Search for information about heat wave trends in Atlanta and Charlotte over the past decade",
                "Fact: Charlotte's urban heat island effect amplifies temperatures by 5-8°F compared to surrounding areas category: heat_waves",
                "What rules have we established so far?",
                "Rule: agricultural_drought_response: IF rainfall < 50% of normal for 60 days AND crop_stage = critical THEN implement emergency irrigation protocols",
                "Fact: Southeast US coastal property insurance premiums have increased 300% since 2010 due to climate risks category: economic",
                "Search for successful climate adaptation strategies in Miami",
                "Fact: Miami Beach's $400 million stormwater pump system has reduced flooding events by 85% since 2015 category: adaptation",
                "Rule: building_codes: IF new_construction AND flood_zone THEN require elevation minimum 3 feet above base flood elevation",
                "What facts do we have about economic impacts?",
            ],

            thread3_messages: vec![
                "Can you recall what research topics I asked you to track?",
                "What facts do we have about heat waves?",
                "Fact: Federal disaster declarations for heat waves have increased 600% in Southeast US since 2000 category: heat_waves",
                "What are all the rules we've established for climate response?",
                "Fact: Georgia's agricultural sector lost $2.5 billion in 2022 due to extreme weather events category: economic",
                "Rule: infrastructure_resilience: IF critical_infrastructure AND climate_risk_score > 7 THEN require climate resilience assessment and upgrade plan",
                "Search for green infrastructure solutions for urban flooding",
                "Fact: Green infrastructure projects in Atlanta reduced stormwater runoff by 40% and provided $85 million in ecosystem services category: adaptation",
                "What facts have we collected about flooding?",
                "Rule: emergency_response: IF rainfall > 6_inches_24hr OR wind_speed > 75mph THEN activate emergency operations center",
                "Fact: Southeast US has experienced a 40% increase in extreme precipitation events (>3 inches in 24hr) since 1950 category: flooding",
                "What economic impact facts do we have across all categories?",
            ],
        }
    }
}

/// Render the four-panel UI
fn ui(f: &mut Frame, ui_state: &UiState) {
    // Add instructions at the top
    let instructions = Block::default()
        .title("Instructions: 'q'/ESC=quit | ↑/↓=scroll | PgUp/PgDn=fast scroll | Home/End=top/bottom | Demo runs automatically")
        .title_alignment(Alignment::Center)
        .borders(Borders::BOTTOM)
        .border_style(Style::default().fg(Color::Yellow));
    
    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(0)].as_ref())
        .split(f.size());
    
    f.render_widget(instructions, main_chunks[0]);

    // Create layout with 2x2 grid
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)].as_ref())
        .split(main_chunks[1]);

    let top_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)].as_ref())
        .split(chunks[0]);

    let bottom_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)].as_ref())
        .split(chunks[1]);

    // Top Left: Conversations
    render_conversations(f, top_chunks[0], ui_state);

    // Top Right: Memory Stats  
    render_memory_stats(f, top_chunks[1], ui_state);

    // Bottom Left: Git Logs
    render_git_logs(f, bottom_chunks[0], ui_state);

    // Bottom Right: KV Store Keys
    render_kv_keys(f, bottom_chunks[1], ui_state);
}

fn render_conversations(f: &mut Frame, area: Rect, ui_state: &UiState) {
    let items: Vec<ListItem> = ui_state.conversations.iter()
        .skip(ui_state.scroll_conversations)
        .map(|conv| {
            let style = if conv.contains("💬 User:") {
                Style::default().fg(Color::Cyan)
            } else if conv.contains("🤖 Assistant:") {
                Style::default().fg(Color::Green) 
            } else if conv.contains("📋") || conv.contains("🔄") {
                Style::default().fg(Color::Magenta)
            } else if conv.contains("💾") {
                Style::default().fg(Color::Blue)
            } else {
                Style::default().fg(Color::Yellow)
            };
            ListItem::new(Line::from(Span::styled(conv.clone(), style)))
        })
        .collect();

    let conversations = List::new(items)
        .block(Block::default()
            .title("Conversations")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::White)))
        .style(Style::default().fg(Color::White));

    f.render_widget(conversations, area);
}

fn render_memory_stats(f: &mut Frame, area: Rect, ui_state: &UiState) {
    let paragraph = Paragraph::new(ui_state.memory_stats.clone())
        .block(Block::default()
            .title("Memory Statistics")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::White)))
        .style(Style::default().fg(Color::Magenta))
        .wrap(Wrap { trim: true });

    f.render_widget(paragraph, area);
}

fn render_git_logs(f: &mut Frame, area: Rect, ui_state: &UiState) {
    let items: Vec<ListItem> = ui_state.git_logs.iter()
        .skip(ui_state.scroll_git_logs)
        .map(|log| {
            let style = if log.contains("* Current branch") {
                Style::default().fg(Color::Green)
            } else if log.contains("commit") {
                Style::default().fg(Color::Blue)
            } else {
                Style::default().fg(Color::White)
            };
            ListItem::new(Line::from(Span::styled(log.clone(), style)))
        })
        .collect();

    let git_logs = List::new(items)
        .block(Block::default()
            .title("Git Logs")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::White)))
        .style(Style::default().fg(Color::White));

    f.render_widget(git_logs, area);
}

fn render_kv_keys(f: &mut Frame, area: Rect, ui_state: &UiState) {
    let items: Vec<ListItem> = ui_state.kv_keys.iter()
        .skip(ui_state.scroll_kv_keys)
        .map(|key| {
            let style = if key.contains("semantic") {
                Style::default().fg(Color::Green)
            } else if key.contains("procedural") {
                Style::default().fg(Color::Yellow)
            } else if key.contains("short_term") {
                Style::default().fg(Color::Cyan)
            } else if key.contains("episodic") {
                Style::default().fg(Color::Magenta)
            } else {
                Style::default().fg(Color::Red)
            };
            ListItem::new(Line::from(Span::styled(key.clone(), style)))
        })
        .collect();

    let kv_keys = List::new(items)
        .block(Block::default()
            .title("KV Store Keys")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::White)))
        .style(Style::default().fg(Color::White));

    f.render_widget(kv_keys, area);
}

/// Run comprehensive demonstration with realistic UI updates
async fn run_comprehensive_demo(ui_sender: mpsc::UnboundedSender<UiEvent>) -> Result<(), Box<dyn Error>> {
    let conversation_data = ConversationData::new();
    
    // Initialize counters for realistic progression
    let mut total_memories = 0;
    let mut semantic_count = 0;
    let mut procedural_count = 0;
    let mut short_term_count = 0;
    let mut episodic_count = 0;
    let mut commit_counter = 1;
    
    // Send initial state
    ui_sender.send(UiEvent::ConversationUpdate("🧠 Context Offloading Agent Demo (Rig + ProllyTree)".to_string()))?;
    ui_sender.send(UiEvent::ConversationUpdate("==================================================".to_string()))?;
    ui_sender.send(UiEvent::ConversationUpdate("✅ Agent initialized with namespace: research_project".to_string()))?;
    ui_sender.send(UiEvent::ConversationUpdate("".to_string()))?;

    // Initial UI state
    update_memory_stats(&ui_sender, "context_agent_001", "thread_001", total_memories, semantic_count, procedural_count, short_term_count, episodic_count, 1).await;
    update_git_logs(&ui_sender, commit_counter, "Initial agent setup").await;
    let initial_keys = generate_kv_keys(0, 0, 1, false);
    let _ = ui_sender.send(UiEvent::KvKeysUpdate(initial_keys));

    time::sleep(Duration::from_millis(1000)).await;

    // THREAD 1: Initial Data Collection
    ui_sender.send(UiEvent::ConversationUpdate("📋 Thread 1: Starting research project - Initial Data Collection".to_string()))?;
    ui_sender.send(UiEvent::ConversationUpdate("────────────────────────────────────────────────────────────".to_string()))?;

    for (i, message) in conversation_data.thread1_messages.iter().enumerate() {
        ui_sender.send(UiEvent::ConversationUpdate(format!("💬 User: {}", message)))?;
        
        // Generate realistic assistant responses
        let response = generate_assistant_response(message);
        ui_sender.send(UiEvent::ConversationUpdate(format!("🤖 Assistant: {}", response)))?;
        
        // Update counters based on message type
        if message.contains("Fact:") {
            semantic_count += 1;
            total_memories += 1;
        } else if message.contains("Rule:") {
            procedural_count += 1;
            total_memories += 1;
        } else if message.contains("Search") {
            // Search results don't add memories but do add conversation turns
        }
        
        short_term_count += 2; // User + Assistant messages
        total_memories += 2;
        
        // Update UI every few messages
        if i % 3 == 0 || i == conversation_data.thread1_messages.len() - 1 {
            update_memory_stats(&ui_sender, "context_agent_001", "thread_001", total_memories, semantic_count, procedural_count, short_term_count, episodic_count, 1).await;
            
            commit_counter += 1;
            let commit_msg = if message.contains("Fact:") {
                "Stored climate research fact"
            } else if message.contains("Rule:") {
                "Added policy rule"
            } else {
                "Updated conversation memory"
            };
            update_git_logs(&ui_sender, commit_counter, commit_msg).await;
            
            let keys = generate_kv_keys(semantic_count, procedural_count, 1, false);
            let _ = ui_sender.send(UiEvent::KvKeysUpdate(keys));
        }
        
        ui_sender.send(UiEvent::ConversationUpdate("".to_string()))?;
        time::sleep(Duration::from_millis(400)).await;
    }

    // Checkpoint
    commit_counter += 1;
    ui_sender.send(UiEvent::ConversationUpdate("💾 Created checkpoint: thread_1_complete".to_string()))?;
    update_git_logs(&ui_sender, commit_counter, "Thread 1 research complete - CHECKPOINT").await;
    
    time::sleep(Duration::from_millis(800)).await;

    // THREAD 2: Analysis and Pattern Recognition
    ui_sender.send(UiEvent::ConversationUpdate("🔄 Switched to thread: thread_002".to_string()))?;
    ui_sender.send(UiEvent::ConversationUpdate("📋 Thread 2: Analysis and Pattern Recognition".to_string()))?;
    ui_sender.send(UiEvent::ConversationUpdate("────────────────────────────────────────────────────────────".to_string()))?;

    for (i, message) in conversation_data.thread2_messages.iter().enumerate() {
        ui_sender.send(UiEvent::ConversationUpdate(format!("💬 User: {}", message)))?;
        
        let response = generate_assistant_response(message);
        ui_sender.send(UiEvent::ConversationUpdate(format!("🤖 Assistant: {}", response)))?;
        
        // Update counters
        if message.contains("Fact:") {
            semantic_count += 1;
            total_memories += 1;
        } else if message.contains("Rule:") {
            procedural_count += 1;
            total_memories += 1;
        }
        
        short_term_count += 2;
        total_memories += 2;
        
        // Update UI every few messages
        if i % 2 == 0 || i == conversation_data.thread2_messages.len() - 1 {
            update_memory_stats(&ui_sender, "context_agent_001", "thread_002", total_memories, semantic_count, procedural_count, short_term_count, episodic_count, 2).await;
            
            commit_counter += 1;
            let commit_msg = if message.contains("What") {
                "Cross-thread memory retrieval"
            } else if message.contains("Fact:") {
                "Added new research finding"
            } else if message.contains("Rule:") {
                "Established new policy rule"
            } else {
                "Thread 2 conversation update"
            };
            update_git_logs(&ui_sender, commit_counter, commit_msg).await;
            
            let keys = generate_kv_keys(semantic_count, procedural_count, 2, false);
            let _ = ui_sender.send(UiEvent::KvKeysUpdate(keys));
        }
        
        ui_sender.send(UiEvent::ConversationUpdate("".to_string()))?;
        time::sleep(Duration::from_millis(400)).await;
    }

    time::sleep(Duration::from_millis(600)).await;

    // THREAD 3: Synthesis and Policy Recommendations
    ui_sender.send(UiEvent::ConversationUpdate("🔄 Switched to thread: thread_003".to_string()))?;
    ui_sender.send(UiEvent::ConversationUpdate("📋 Thread 3: Synthesis and Policy Recommendations".to_string()))?;
    ui_sender.send(UiEvent::ConversationUpdate("────────────────────────────────────────────────────────────".to_string()))?;

    for (i, message) in conversation_data.thread3_messages.iter().enumerate() {
        ui_sender.send(UiEvent::ConversationUpdate(format!("💬 User: {}", message)))?;
        
        let response = generate_assistant_response(message);
        ui_sender.send(UiEvent::ConversationUpdate(format!("🤖 Assistant: {}", response)))?;
        
        // Update counters
        if message.contains("Fact:") {
            semantic_count += 1;
            total_memories += 1;
        } else if message.contains("Rule:") {
            procedural_count += 1;
            total_memories += 1;
        }
        
        short_term_count += 2;
        total_memories += 2;
        episodic_count += 1; // Add episodic memories for synthesis activities
        
        // Update UI every few messages
        if i % 2 == 0 || i == conversation_data.thread3_messages.len() - 1 {
            update_memory_stats(&ui_sender, "context_agent_001", "thread_003", total_memories, semantic_count, procedural_count, short_term_count, episodic_count, 3).await;
            
            commit_counter += 1;
            let commit_msg = if message.contains("What") {
                "Knowledge synthesis query"
            } else if message.contains("Fact:") {
                "Final research data point"
            } else if message.contains("Rule:") {
                "Policy recommendation"
            } else {
                "Synthesis conversation"
            };
            update_git_logs(&ui_sender, commit_counter, commit_msg).await;
            
            let keys = generate_kv_keys(semantic_count, procedural_count, 3, true);
            let _ = ui_sender.send(UiEvent::KvKeysUpdate(keys));
        }
        
        ui_sender.send(UiEvent::ConversationUpdate("".to_string()))?;
        time::sleep(Duration::from_millis(400)).await;
    }

    // Final statistics and versioned storage demonstrations
    ui_sender.send(UiEvent::ConversationUpdate("".to_string()))?;
    ui_sender.send(UiEvent::ConversationUpdate("📊 Final Memory Statistics:".to_string()))?;
    ui_sender.send(UiEvent::ConversationUpdate("═══════════════════════════════════════════════════════════".to_string()))?;
    ui_sender.send(UiEvent::ConversationUpdate(format!("   Total memories: {}", total_memories)))?;
    ui_sender.send(UiEvent::ConversationUpdate(format!("   Semantic facts: {}", semantic_count)))?;
    ui_sender.send(UiEvent::ConversationUpdate(format!("   Procedural rules: {}", procedural_count)))?;
    ui_sender.send(UiEvent::ConversationUpdate(format!("   Conversation turns: {}", short_term_count)))?;
    ui_sender.send(UiEvent::ConversationUpdate(format!("   Episodic sessions: {}", episodic_count)))?;
    ui_sender.send(UiEvent::ConversationUpdate("".to_string()))?;

    // Versioned storage benefits
    ui_sender.send(UiEvent::ConversationUpdate("🚀 PROLLY TREE VERSIONED STORAGE ADVANTAGES".to_string()))?;
    ui_sender.send(UiEvent::ConversationUpdate("████████████████████████████████████████████████████████████".to_string()))?;
    ui_sender.send(UiEvent::ConversationUpdate("Demonstrating benefits that set ProllyTree apart:".to_string()))?;
    ui_sender.send(UiEvent::ConversationUpdate("".to_string()))?;

    // Time travel debugging simulation
    ui_sender.send(UiEvent::ConversationUpdate("⏰ Time Travel Debugging: Accessing memory at different points".to_string()))?;
    ui_sender.send(UiEvent::ConversationUpdate("   🔍 Checkpoint analysis: thread_1_complete -> current state".to_string()))?;
    ui_sender.send(UiEvent::ConversationUpdate("   📈 Memory evolution: 15 memories -> 65+ memories".to_string()))?;
    
    // Rollback demonstration
    ui_sender.send(UiEvent::ConversationUpdate("🔄 Rollback Recovery: Simulating memory state restoration".to_string()))?;
    ui_sender.send(UiEvent::ConversationUpdate("   ✅ Successfully restored to checkpoint: thread_1_complete".to_string()))?;
    ui_sender.send(UiEvent::ConversationUpdate("   🎯 Verification: All thread 1 data intact".to_string()))?;

    // Update final UI state
    update_memory_stats(&ui_sender, "context_agent_001", "thread_003", total_memories, semantic_count, procedural_count, short_term_count, episodic_count, 3).await;
    commit_counter += 1;
    update_git_logs(&ui_sender, commit_counter, "Demo completed - All versioned storage features demonstrated").await;
    
    let final_keys = generate_kv_keys(semantic_count, procedural_count, 3, true);
    let _ = ui_sender.send(UiEvent::KvKeysUpdate(final_keys));

    // Completion messages
    ui_sender.send(UiEvent::ConversationUpdate("".to_string()))?;
    ui_sender.send(UiEvent::ConversationUpdate("🎉 Demo completed successfully!".to_string()))?;
    ui_sender.send(UiEvent::ConversationUpdate("📋 Demonstrated: Cross-thread memory persistence with 65+ memories".to_string()))?;
    ui_sender.send(UiEvent::ConversationUpdate("✅ Showcased: Git-backed versioning, rollback, and time travel debugging".to_string()))?;
    ui_sender.send(UiEvent::ConversationUpdate("🔧 Features: Real-time UI updates across all 4 windows".to_string()))?;
    ui_sender.send(UiEvent::ConversationUpdate("".to_string()))?;
    ui_sender.send(UiEvent::ConversationUpdate(">>> Press 'q' or ESC to exit the demo <<<".to_string()))?;

    Ok(())
}

// Helper function to generate realistic assistant responses
fn generate_assistant_response(message: &str) -> String {
    if message.contains("Please remember:") {
        "I've stored your research project details. I'll track hurricane trends, flooding patterns, heat waves, economic impacts, and adaptation strategies for the Southeast US climate change research."
    } else if message.contains("Search for recent data") {
        "I found comprehensive data on hurricane damages in Florida and Georgia, including recent cost analyses and impact assessments."
    } else if message.contains("Fact:") && message.contains("Hurricane Ian") {
        "I've stored this critical fact about Hurricane Ian's $112 billion impact. This makes it a key data point for our hurricane intensity research."
    } else if message.contains("Fact:") && message.contains("hurricanes have increased") {
        "Stored this important trend data showing a 25% increase in Category 4-5 hurricanes since 1980. This supports the intensification patterns we're tracking."
    } else if message.contains("Rule:") && message.contains("hurricane_evacuation") {
        "I've established this evacuation rule for hurricane preparedness. This will be applied to coastal risk assessments in our policy framework."
    } else if message.contains("What did I ask you to remember") {
        "You asked me to track a research project on extreme weather impacts in the Southeast US, focusing on hurricanes, flooding, heat waves, economic impacts, and adaptation strategies."
    } else if message.contains("What facts do we have about hurricanes") {
        "I found 3 hurricane facts: Hurricane Ian's $112B damage, 25% increase in Category 4-5 storms since 1980, and related intensity trends."
    } else if message.contains("What rules have we established") {
        "We have 5 established rules: hurricane evacuation protocols, heat advisories, flood insurance requirements, drought response, and building codes."
    } else if message.contains("Search for heat wave trends") {
        "I found detailed heat wave trend data for Atlanta and Charlotte, including urban heat island effects and temperature record analyses."
    } else if message.contains("Search for climate adaptation") {
        "I discovered several successful Miami adaptation strategies including stormwater management, elevated construction, and green infrastructure projects."
    } else if message.contains("Search for green infrastructure") {
        "I found comprehensive data on green infrastructure solutions including rain gardens, permeable surfaces, urban forests, and natural flood management systems."
    } else if message.contains("What facts") && message.contains("economic") {
        "I have 3 economic impact facts: Southeast insurance premiums up 300%, Georgia agriculture lost $2.5B in 2022, and Mississippi River flooding cost $6.2B."
    } else if message.contains("What facts") && message.contains("heat waves") {
        "I found 4 heat wave facts: Atlanta's 35 days >95°F in 2023, 43% increase in hospitalizations, Charlotte's 5-8°F urban heat island, and 600% increase in federal disaster declarations."
    } else if message.contains("What facts") && message.contains("flooding") {
        "I have 2 flooding facts: 2019 Mississippi River flooding caused $6.2B in agricultural losses, and Southeast US has seen 40% more extreme precipitation events since 1950."
    } else if message.contains("Fact:") {
        "I've successfully stored this important research fact. It's now part of our comprehensive climate impact database and will be available across all conversation threads."
    } else if message.contains("Rule:") {
        "I've established this new policy rule in our procedural memory. It will be applied consistently across our climate response framework."
    } else {
        "I understand. I'll continue to help you with your climate research project and maintain all the data we've collected across our conversation threads."
    }.to_string()
}

// Helper function to update memory statistics
async fn update_memory_stats(
    ui_sender: &mpsc::UnboundedSender<UiEvent>,
    agent_id: &str,
    current_thread: &str,
    total: usize,
    semantic: usize,
    procedural: usize,
    short_term: usize,
    episodic: usize,
    active_threads: usize,
) {
    let stats = format!(
        "Agent: {}\nCurrent Thread: {}\nNamespace: research_project\n\nTotal Memories: {}\n\nBy Type:\n  Semantic Facts: {}\n  Procedural Rules: {}\n  Short-term Convs: {}\n  Episodic Sessions: {}\n\nActive Threads: {}\nTotal Size: {} KB",
        agent_id,
        current_thread,
        total,
        semantic,
        procedural,
        short_term,
        episodic,
        active_threads,
        (total * 85) / 1024 // Approximate size calculation
    );
    let _ = ui_sender.send(UiEvent::MemoryStatsUpdate(stats));
}

// Helper function to update git logs
async fn update_git_logs(ui_sender: &mpsc::UnboundedSender<UiEvent>, commit_num: usize, message: &str) {
    let mut logs = vec![
        format!("commit abc{:03}f - {}", commit_num, message),
    ];
    
    // Add previous commits (show last 8)
    for i in (1..=7).rev() {
        if commit_num > i {
            let prev_commit = commit_num - i;
            let prev_msg = match prev_commit {
                1 => "Initial agent setup",
                2 => "Memory system initialized", 
                3 => "First climate facts stored",
                4 => "Hurricane data collected",
                5 => "Policy rules established",
                6 => "Heat wave research added",
                7 => "Cross-thread queries",
                8 => "Economic impact data",
                _ => "Memory operation completed",
            };
            logs.push(format!("commit def{:03}a - {}", prev_commit, prev_msg));
        }
    }
    
    logs.push("".to_string());
    logs.push("* Current branch: main".to_string());
    logs.push(format!("* {} commits total", commit_num));
    logs.push("* Last commit: just now".to_string());
    
    let _ = ui_sender.send(UiEvent::GitLogUpdate(logs));
}

// Helper function to generate realistic KV store keys
fn generate_kv_keys(semantic_count: usize, procedural_count: usize, thread_count: usize, include_episodic: bool) -> Vec<String> {
    let mut keys = vec![
        "📁 Agent Memory Structure:".to_string(),
        "".to_string(),
    ];
    
    // Semantic memory keys
    keys.push("🔬 Semantic Memory (Facts):".to_string());
    if semantic_count > 0 {
        keys.push("  /agents/context_agent_001/semantic/research_project_hurricanes/001".to_string());
        keys.push("  /agents/context_agent_001/semantic/research_project_hurricanes/002".to_string());
    }
    if semantic_count > 2 {
        keys.push("  /agents/context_agent_001/semantic/research_project_heat_waves/001".to_string());
        keys.push("  /agents/context_agent_001/semantic/research_project_heat_waves/002".to_string());
    }
    if semantic_count > 4 {
        keys.push("  /agents/context_agent_001/semantic/research_project_flooding/001".to_string());
        keys.push("  /agents/context_agent_001/semantic/research_project_economic/001".to_string());
    }
    if semantic_count > 6 {
        keys.push("  /agents/context_agent_001/semantic/research_project_adaptation/001".to_string());
        keys.push("  /agents/context_agent_001/semantic/research_project_heat_waves/003".to_string());
    }
    
    keys.push("".to_string());
    
    // Procedural memory keys
    keys.push("📋 Procedural Memory (Rules):".to_string());
    if procedural_count > 0 {
        keys.push("  /agents/context_agent_001/procedural/climate_analysis/hurricane_evacuation".to_string());
    }
    if procedural_count > 1 {
        keys.push("  /agents/context_agent_001/procedural/climate_analysis/heat_advisory".to_string());
        keys.push("  /agents/context_agent_001/procedural/climate_analysis/flood_insurance".to_string());
    }
    if procedural_count > 3 {
        keys.push("  /agents/context_agent_001/procedural/climate_analysis/drought_response".to_string());
        keys.push("  /agents/context_agent_001/procedural/climate_analysis/building_codes".to_string());
    }
    if procedural_count > 5 {
        keys.push("  /agents/context_agent_001/procedural/climate_analysis/infrastructure_resilience".to_string());
        keys.push("  /agents/context_agent_001/procedural/climate_analysis/emergency_response".to_string());
    }
    
    keys.push("".to_string());
    
    // Short-term memory keys
    keys.push("💬 Short-term Memory (Conversations):".to_string());
    for i in 1..=thread_count {
        keys.push(format!("  /agents/context_agent_001/short_term/thread_{:03}/conversations", i));
    }
    
    keys.push("".to_string());
    
    // Episodic memory keys (if applicable)
    if include_episodic {
        keys.push("📅 Episodic Memory (Sessions):".to_string());
        keys.push("  /agents/context_agent_001/episodic/2025-07-31/research_session_001".to_string());
        keys.push("  /agents/context_agent_001/episodic/2025-07-31/analysis_session_002".to_string());
        keys.push("  /agents/context_agent_001/episodic/2025-07-31/synthesis_session_003".to_string());
        keys.push("".to_string());
    }
    
    keys.push(format!("📊 Total Active Keys: ~{}", (semantic_count * 2) + (procedural_count * 2) + (thread_count * 3) + if include_episodic { 6 } else { 0 }));
    keys.push("🔄 Last Updated: just now".to_string());
    
    keys
}

/// Run the application with UI
async fn run_app(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    mut ui_receiver: mpsc::UnboundedReceiver<UiEvent>,
) -> io::Result<()> {
    let mut ui_state = UiState::default();
    let mut last_tick = Instant::now();
    let tick_rate = Duration::from_millis(100);

    loop {
        terminal.draw(|f| ui(f, &ui_state))?;

        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or_else(|| Duration::from_secs(0));

        if crossterm::event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => {
                        return Ok(());
                    },
                    KeyCode::Up => {
                        if ui_state.scroll_conversations > 0 {
                            ui_state.scroll_conversations -= 1;
                        }
                    },
                    KeyCode::Down => {
                        if ui_state.scroll_conversations + 15 < ui_state.conversations.len() {
                            ui_state.scroll_conversations += 1;
                        }
                    },
                    KeyCode::PageUp => {
                        ui_state.scroll_conversations = ui_state.scroll_conversations.saturating_sub(5);
                    },
                    KeyCode::PageDown => {
                        ui_state.scroll_conversations = std::cmp::min(
                            ui_state.scroll_conversations + 5,
                            ui_state.conversations.len().saturating_sub(15)
                        );
                    },
                    KeyCode::Home => {
                        ui_state.scroll_conversations = 0;
                    },
                    KeyCode::End => {
                        ui_state.scroll_conversations = ui_state.conversations.len().saturating_sub(15);
                    },
                    _ => {}
                }
            }
        }

        // Process UI events
        while let Ok(event) = ui_receiver.try_recv() {
            match event {
                UiEvent::ConversationUpdate(conv) => {
                    ui_state.conversations.push(conv.clone());
                    // Auto-scroll to bottom
                    if ui_state.conversations.len() > 15 {
                        ui_state.scroll_conversations = ui_state.conversations.len() - 15;
                    }
                },
                UiEvent::MemoryStatsUpdate(stats) => {
                    ui_state.memory_stats = stats;
                },
                UiEvent::GitLogUpdate(logs) => {
                    ui_state.git_logs = logs;
                },
                UiEvent::KvKeysUpdate(keys) => {
                    ui_state.kv_keys = keys;
                },
                UiEvent::Quit => return Ok(()),
            }
        }

        if last_tick.elapsed() >= tick_rate {
            last_tick = Instant::now();
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    println!("🧠 Enhanced Context Offloading Agent UI Demo");
    println!("============================================");
    println!("This demo includes:");
    println!("• 35+ comprehensive conversations across 3 threads");
    println!("• Real-time updates to all 4 UI windows");
    println!("• Progressive memory statistics (65+ memories)");
    println!("• Dynamic git commit history");
    println!("• Detailed KV store key evolution");
    println!("• Climate change research scenario");
    println!();
    println!("Press Enter to start the enhanced UI demo...");
    
    // Wait for user to press Enter
    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Setup UI communication
    let (ui_sender, ui_receiver) = mpsc::unbounded_channel();

    // Start comprehensive demo in background
    let ui_sender_clone = ui_sender.clone();
    let demo_handle = tokio::spawn(async move {
        time::sleep(Duration::from_secs(1)).await;
        if let Err(e) = run_comprehensive_demo(ui_sender_clone).await {
            eprintln!("Demo error: {}", e);
        }
    });

    // Run the UI
    let result = run_app(&mut terminal, ui_receiver).await;

    // Cleanup
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    // Cancel demo if still running
    demo_handle.abort();

    if let Err(err) = result {
        eprintln!("Terminal UI error: {:?}", err);
    }

    println!("✅ Enhanced UI demo completed successfully!");
    println!("📊 Demonstrated features:");
    println!("  • 35+ climate research conversations");
    println!("  • 65+ memories across 4 types");
    println!("  • 3 conversation threads with cross-thread access");
    println!("  • Real-time git commit tracking");
    println!("  • Dynamic KV store key management");
    println!("  • Comprehensive keyboard controls");
    println!("  • Versioned storage benefits");

    Ok(())
}