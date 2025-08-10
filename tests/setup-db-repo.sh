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
DEMO_DIR="/tmp/multi-dataset-demo"
PROLLY_BIN="$PROJECT_ROOT/target/debug/git-prolly"
UI_BIN="$PROJECT_ROOT/target/debug/prolly-ui"

echo "🚀 Setting up multi-dataset demo with multiple branches..."
echo "📁 Demo directory: $DEMO_DIR"
echo "🔧 Project root: $PROJECT_ROOT"

# Clean up and create fresh demo directory
rm -rf "$DEMO_DIR"
mkdir -p "$DEMO_DIR"

cd "$DEMO_DIR"

# Initialize git repository first
git init

# Configure git for the dataset's individual repository
git config user.name "Demo User"
git config user.email "demo@prollytree.com"

# Create datasets as individual Git repositories for multi-dataset visualization
DATASETS=("customers" "products" "orders" "inventory")

for dataset in "${DATASETS[@]}"; do
    echo ""
    echo "📊 Creating dataset: $dataset"

    # Create dataset directory structure
    mkdir -p "$dataset"
    cd "$dataset"

    # Initialize git-prolly in the data subdirectory
    "$PROLLY_BIN" init

    cd "$DEMO_DIR"
done

echo ""
echo "🔄 Adding data to each dataset and creating branches..."

# Dataset 1: Customers
echo "👥 Populating customers dataset..."

# Main branch data
(cd "$DEMO_DIR/customers" && "$PROLLY_BIN" set "customer:1001" "Alice Johnson")
(cd "$DEMO_DIR/customers" && "$PROLLY_BIN" set "customer:1002" "Bob Smith")
(cd "$DEMO_DIR/customers" && "$PROLLY_BIN" set "customer:1003" "Carol Davis")
(cd "$DEMO_DIR/customers" && "$PROLLY_BIN" commit -m "Initial customer data")

(cd "$DEMO_DIR/customers" && "$PROLLY_BIN" set "customer:1004" "David Wilson")
(cd "$DEMO_DIR/customers" && "$PROLLY_BIN" set "customer:1005" "Emma Brown")
(cd "$DEMO_DIR/customers" && "$PROLLY_BIN" commit -m "Add more customers")

# Create feature branch
(cd "$DEMO_DIR/customers" && git checkout -b feature/vip-customers)
(cd "$DEMO_DIR/customers" && "$PROLLY_BIN" set "customer:1001" "Alice Johnson (VIP)")
(cd "$DEMO_DIR/customers" && "$PROLLY_BIN" set "customer:1006" "Frank Miller (VIP)")
(cd "$DEMO_DIR/customers" && "$PROLLY_BIN" commit -m "Add VIP customer status")

(cd "$DEMO_DIR/customers" && "$PROLLY_BIN" set "customer:1007" "Grace Lee (VIP)")
(cd "$DEMO_DIR/customers" && "$PROLLY_BIN" commit -m "Add Grace as VIP customer")

# Create development branch
(cd "$DEMO_DIR/customers" && git checkout -b develop)
(cd "$DEMO_DIR/customers" && "$PROLLY_BIN" set "customer:1008" "Henry Taylor")
(cd "$DEMO_DIR/customers" && "$PROLLY_BIN" set "customer:1009" "Ivy Chen")
(cd "$DEMO_DIR/customers" && "$PROLLY_BIN" commit -m "Development customer additions")

# Back to main
(cd "$DEMO_DIR/customers" && git checkout main)

# Dataset 2: Products
echo "🛍️  Populating products dataset..."

# Main branch data
(cd "$DEMO_DIR/products" && "$PROLLY_BIN" set "product:2001" "Laptop Pro 15")
(cd "$DEMO_DIR/products" && "$PROLLY_BIN" set "product:2002" "Wireless Mouse")
(cd "$DEMO_DIR/products" && "$PROLLY_BIN" set "product:2003" "Mechanical Keyboard")
(cd "$DEMO_DIR/products" && "$PROLLY_BIN" commit -m "Initial product catalog")

(cd "$DEMO_DIR/products" && "$PROLLY_BIN" set "product:2004" "USB-C Hub")
(cd "$DEMO_DIR/products" && "$PROLLY_BIN" set "product:2005" "Monitor Stand")
(cd "$DEMO_DIR/products" && "$PROLLY_BIN" commit -m "Add accessories")

# Create feature branch
(cd "$DEMO_DIR/products" && git checkout -b feature/new-products)
(cd "$DEMO_DIR/products" && "$PROLLY_BIN" set "product:2006" "Gaming Headset")
(cd "$DEMO_DIR/products" && "$PROLLY_BIN" set "product:2007" "Webcam HD")
(cd "$DEMO_DIR/products" && "$PROLLY_BIN" commit -m "Add gaming and streaming products")

(cd "$DEMO_DIR/products" && "$PROLLY_BIN" set "product:2008" "Tablet 10 inch")
(cd "$DEMO_DIR/products" && "$PROLLY_BIN" commit -m "Add tablet to catalog")

# Create release branch
(cd "$DEMO_DIR/products" && git checkout -b release/v1.2)
(cd "$DEMO_DIR/products" && "$PROLLY_BIN" set "product:2001" "Laptop Pro 15 (Updated)")
(cd "$DEMO_DIR/products" && "$PROLLY_BIN" set "product:2009" "Laptop Pro 13")
(cd "$DEMO_DIR/products" && "$PROLLY_BIN" commit -m "Product updates for v1.2 release")

# Back to main
(cd "$DEMO_DIR/products" && git checkout main)

# Dataset 3: Orders
echo "📦 Populating orders dataset..."

# Main branch data
(cd "$DEMO_DIR/orders" && "$PROLLY_BIN" set "order:3001" "customer:1001|product:2001|qty:1")
(cd "$DEMO_DIR/orders" && "$PROLLY_BIN" set "order:3002" "customer:1002|product:2002|qty:2")
(cd "$DEMO_DIR/orders" && "$PROLLY_BIN" set "order:3003" "customer:1003|product:2003|qty:1")
(cd "$DEMO_DIR/orders" && "$PROLLY_BIN" commit -m "Initial orders")

(cd "$DEMO_DIR/orders" && "$PROLLY_BIN" set "order:3004" "customer:1001|product:2004|qty:1")
(cd "$DEMO_DIR/orders" && "$PROLLY_BIN" set "order:3005" "customer:1004|product:2001|qty:1")
(cd "$DEMO_DIR/orders" && "$PROLLY_BIN" commit -m "Additional orders")

# Create processing branch
(cd "$DEMO_DIR/orders" && git checkout -b processing/batch-1)
(cd "$DEMO_DIR/orders" && "$PROLLY_BIN" set "order:3001" "customer:1001|product:2001|qty:1|status:shipped")
(cd "$DEMO_DIR/orders" && "$PROLLY_BIN" set "order:3002" "customer:1002|product:2002|qty:2|status:shipped")
(cd "$DEMO_DIR/orders" && "$PROLLY_BIN" commit -m "Update order status - batch 1 shipped")

(cd "$DEMO_DIR/orders" && "$PROLLY_BIN" set "order:3003" "customer:1003|product:2003|qty:1|status:delivered")
(cd "$DEMO_DIR/orders" && "$PROLLY_BIN" commit -m "Order 3003 delivered")

# Create urgent branch
(cd "$DEMO_DIR/orders" && git checkout -b urgent/priority-orders)
(cd "$DEMO_DIR/orders" && "$PROLLY_BIN" set "order:3006" "customer:1006|product:2001|qty:1|priority:high")
(cd "$DEMO_DIR/orders" && "$PROLLY_BIN" set "order:3007" "customer:1007|product:2002|qty:3|priority:high")
(cd "$DEMO_DIR/orders" && "$PROLLY_BIN" commit -m "Add priority orders")

# Back to main
(cd "$DEMO_DIR/orders" && git checkout main)

# Dataset 4: Inventory
echo "📋 Populating inventory dataset..."

# Main branch data
(cd "$DEMO_DIR/inventory" && "$PROLLY_BIN" set "inventory:2001" "warehouse:A|stock:50|reserved:5")
(cd "$DEMO_DIR/inventory" && "$PROLLY_BIN" set "inventory:2002" "warehouse:A|stock:200|reserved:10")
(cd "$DEMO_DIR/inventory" && "$PROLLY_BIN" set "inventory:2003" "warehouse:B|stock:75|reserved:3")
(cd "$DEMO_DIR/inventory" && "$PROLLY_BIN" commit -m "Initial inventory levels")

(cd "$DEMO_DIR/inventory" && "$PROLLY_BIN" set "inventory:2004" "warehouse:B|stock:30|reserved:2")
(cd "$DEMO_DIR/inventory" && "$PROLLY_BIN" set "inventory:2005" "warehouse:A|stock:15|reserved:1")
(cd "$DEMO_DIR/inventory" && "$PROLLY_BIN" commit -m "Add new product inventory")

# Create restock branch
(cd "$DEMO_DIR/inventory" && git checkout -b operations/restock)
(cd "$DEMO_DIR/inventory" && "$PROLLY_BIN" set "inventory:2001" "warehouse:A|stock:150|reserved:5")
(cd "$DEMO_DIR/inventory" && "$PROLLY_BIN" set "inventory:2003" "warehouse:B|stock:125|reserved:3")
(cd "$DEMO_DIR/inventory" && "$PROLLY_BIN" commit -m "Restock warehouse inventory")

(cd "$DEMO_DIR/inventory" && "$PROLLY_BIN" set "inventory:2006" "warehouse:C|stock:100|reserved:0")
(cd "$DEMO_DIR/inventory" && "$PROLLY_BIN" set "inventory:2007" "warehouse:C|stock:80|reserved:0")
(cd "$DEMO_DIR/inventory" && "$PROLLY_BIN" commit -m "Add warehouse C inventory")

# Create audit branch
(cd "$DEMO_DIR/inventory" && git checkout -b audit/quarterly-review)
(cd "$DEMO_DIR/inventory" && "$PROLLY_BIN" set "inventory:2002" "warehouse:A|stock:195|reserved:10|audited:2024-08-09")
(cd "$DEMO_DIR/inventory" && "$PROLLY_BIN" set "inventory:2004" "warehouse:B|stock:28|reserved:2|audited:2024-08-09")
(cd "$DEMO_DIR/inventory" && "$PROLLY_BIN" commit -m "Quarterly inventory audit updates")

# Back to main
(cd "$DEMO_DIR/inventory" && git checkout main)

## Generate the HTML output in the expected location
echo "📊 Generating unified multi-dataset visualization..."
"$UI_BIN" "$DEMO_DIR/customers" \
  --dataset "Products:$DEMO_DIR/products" \
  --dataset "Orders:$DEMO_DIR/orders" \
  --dataset "Inventory:$DEMO_DIR/inventory" \
  -o "$PROJECT_ROOT/examples/prollytree-ui.html"

echo ""
echo "✅ HTML visualization generated successfully!"
echo "  📄 Output file: $PROJECT_ROOT/examples/prollytree-ui.html"
echo "  🌐 Multi-dataset view with comprehensive git commit details"
echo "  📊 Features: Dataset switching, branch filtering, commit diff details"

cd "$PROJECT_ROOT"

open  "$PROJECT_ROOT/examples/prollytree-ui.html"
