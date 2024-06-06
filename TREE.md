# Prolly Tree in Rust

A Prolly Tree (probabilistic tree) implementation in Rust for efficient storage, retrieval, and modification of ordered data with integrity guarantees.

```text


///  Sample Prolly Tree with 3 levels on the root page:
///
///                            [Root Page Level 2]
///                              /            \
///           [Page Level 1]                   [Page Level 1]
///             /      \                         /      \
///     [Node1][Node2][Node3]             [Node4][Node5][Node6]
///       /          |       \               |          |         \
///     Page       Page     Page          Page       Page       Page
///     (Level 0)(Level 0)(Level 0)      (Level 0)(Level 0)    (Level 0)
///       |         |         |              |         |         |
///     [NodeA][NodeB][NodeC]           [NodeD][NodeE][NodeF]
///
///
///  Sample Prolly Tree with 3 levels on the root page:
///
///                                  [Root Page Level 3]
///                                        /      \
///                            [Page Level 2]     [Page Level 2]
///                            /         \              \
///                  [Node1][Node2]   [Node3]         [Node4]
///                     /       |         \               \
///                Page        Page      Page           Page
///              (Level 1)  (Level 1)  (Level 1)     (Level 1)
///               /    \           \      |             |      \
///      [NodeA][NodeB]       [NodeC]  [NodeD]     [NodeE][NodeF]
///        /      \                 |         \             \
///      Page   Page           Page       Page         Page
///     (Level 0)(Level 0)  (Level 0)   (Level 0)     (Level 0)
///        |          |            |             |             |
///     [NodeG]  [NodeH]     [NodeI]    [NodeJ]   [NodeK]   [NodeL][NodeM]
///
```
