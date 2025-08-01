# Conversation Data Files

This folder contains JSON files that define the conversation scenarios for the agent-context demo.

## File Format

The conversation data should be in JSON format with three message arrays:

```json
{
  "thread1_messages": [
    "Message 1 for thread 1",
    "Message 2 for thread 1",
    "..."
  ],
  "thread2_messages": [
    "Message 1 for thread 2",
    "..."
  ],
  "thread3_messages": [
    "Message 1 for thread 3",
    "..."
  ]
}
```

## Special Message Types

The demo recognizes special message patterns:

- **Facts**: `"Fact: [description] category: [category_name]"`
  - Example: `"Fact: Water boils at 100°C category: physics"`

- **Rules**: `"Rule: [rule_name]: IF [condition] THEN [action]"`
  - Example: `"Rule: safety_check: IF temperature > 80C THEN activate cooling"`

- **Search**: `"Search for [query]"`
  - Example: `"Search for information about renewable energy"`

- **Queries**: Questions that trigger memory recall
  - Example: `"What facts do we have about physics?"`

## Available Files

- `conversation_data.json` - Default climate research scenario (comprehensive)
- `conversation_data_simple.json` - Simple technology scenario (minimal)

## File Loading Order

The demo automatically tries to load conversation files in this order:
1. `examples/data/conversation_data.json` (primary)
2. `data/conversation_data.json` (when running from project root)
3. `examples/data/conversation_data_simple.json` (simple fallback)
4. `conversation_data.json` (legacy fallback)

If no files are found or have JSON parsing errors, the demo will exit with an error message.

## Creating Custom Scenarios

1. Copy one of the existing JSON files in this folder
2. Modify the messages to fit your scenario
3. Save with a descriptive name
4. Update the file path in the demo code if needed
5. Run the demo to see your custom conversations

## Thread Structure

- **Thread 1**: Data collection phase - gathering facts and establishing rules
- **Thread 2**: Analysis and cross-referencing phase - querying existing knowledge
- **Thread 3**: Synthesis and conclusions phase - building final insights

This structure demonstrates the agent's ability to maintain context across different conversation threads while building up knowledge over time.

## Tips

- Keep messages concise but meaningful
- Mix different message types (facts, rules, searches, queries) 
- Use consistent category names for better recall
- Test your JSON syntax before running the demo