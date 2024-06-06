# Prolly Tree Data Structure

A Prolly Tree (probabilistic tree) implementation in Rust for efficient storage, retrieval, and modification of ordered data with integrity guarantees.

## Examples

### Sample Prolly Tree node

```text
                      [Node]
                  /     |     \
               Key: K1 Value: V1 Hash: H1
                                /     \
                      Child Hash1    Child Hash2
                        /               \
                 [Child Node]       [Child Node]
                    /    |    \          /     |     \
                  Key  Value  Hash     Key   Value  Hash
                  K2    V2     H2      K3     V3     H3
    
```
- [Node]: Represents a single node in the Merkle search tree.
- Key: The key of the node, used for organizing and retrieving nodes within the tree.
- Value: The actual data value associated with the key.
- Hash: The cryptographic hash of the node, computed from its key, value, and child hashes.
- Child Hash1 and Child Hash2: Hashes of the child nodes.
- [Child Node]: Represents the child nodes, which also have keys, values, and hashes.


### Sample Prolly Tree Pages and Nodes

```text
                                      [Root Page Level 3]
                                            /      \
                                [Page Level 2]     [Page Level 2]
                                /         \              \
                      [Node1][Node2]   [Node3]         [Node4]
                         /       |         \               \
                    Page        Page      Page           Page
                  (Level 1)  (Level 1)  (Level 1)     (Level 1)
                   /    \           \      |             |      \
          [NodeA][NodeB]       [NodeC]  [NodeD]     [NodeE][NodeF]
            /      \                 |         \             \
          Page   Page           Page       Page         Page
         (Level 0)(Level 0)  (Level 0)   (Level 0)     (Level 0)
            |          |            |             |             |
         [NodeG]  [NodeH]     [NodeI]    [NodeJ]   [NodeK]   [NodeL][NodeM]
    
```
