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
DATASETS=("customers" "products" "orders" "inventory" "suppliers" "employees" "analytics" "reviews" "shipping")

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

# Dataset 5: Suppliers
echo "🏭 Populating suppliers dataset..."

# Main branch data
(cd "$DEMO_DIR/suppliers" && "$PROLLY_BIN" set "supplier:5001" "Tech Solutions Inc|contact:john@techsol.com|rating:A")
(cd "$DEMO_DIR/suppliers" && "$PROLLY_BIN" set "supplier:5002" "Hardware Direct|contact:sales@hwdirect.com|rating:B+")
(cd "$DEMO_DIR/suppliers" && "$PROLLY_BIN" set "supplier:5003" "Components Plus|contact:orders@compplus.com|rating:A-")
(cd "$DEMO_DIR/suppliers" && "$PROLLY_BIN" commit -m "Initial supplier database")

(cd "$DEMO_DIR/suppliers" && "$PROLLY_BIN" set "supplier:5004" "Global Electronics|contact:info@globalelec.com|rating:B")
(cd "$DEMO_DIR/suppliers" && "$PROLLY_BIN" set "supplier:5005" "Premium Parts|contact:premium@parts.com|rating:A+")
(cd "$DEMO_DIR/suppliers" && "$PROLLY_BIN" commit -m "Add additional suppliers")

# Create vendor management branch
(cd "$DEMO_DIR/suppliers" && git checkout -b vendor/management)
(cd "$DEMO_DIR/suppliers" && "$PROLLY_BIN" set "supplier:5001" "Tech Solutions Inc|contact:john@techsol.com|rating:A+|contract:2024-2026")
(cd "$DEMO_DIR/suppliers" && "$PROLLY_BIN" set "supplier:5006" "Swift Logistics|contact:dispatch@swiftlog.com|rating:A|specialty:shipping")
(cd "$DEMO_DIR/suppliers" && "$PROLLY_BIN" commit -m "Update supplier contracts and ratings")

(cd "$DEMO_DIR/suppliers" && "$PROLLY_BIN" set "supplier:5007" "Quality Assurance Co|contact:qa@qualityco.com|rating:A+|specialty:testing")
(cd "$DEMO_DIR/suppliers" && "$PROLLY_BIN" commit -m "Add QA supplier partnership")

# Create procurement branch
(cd "$DEMO_DIR/suppliers" && git checkout -b procurement/q3-2024)
(cd "$DEMO_DIR/suppliers" && "$PROLLY_BIN" set "supplier:5008" "Budget Components|contact:bulk@budgetcomp.com|rating:B-|volume:high")
(cd "$DEMO_DIR/suppliers" && "$PROLLY_BIN" set "supplier:5009" "Specialty Materials|contact:special@materials.com|rating:A|volume:low")
(cd "$DEMO_DIR/suppliers" && "$PROLLY_BIN" commit -m "Q3 procurement supplier additions")

# Back to main
(cd "$DEMO_DIR/suppliers" && git checkout main)

# Dataset 6: Employees
echo "👨‍💼 Populating employees dataset..."

# Main branch data
(cd "$DEMO_DIR/employees" && "$PROLLY_BIN" set "employee:6001" "Alice Johnson|department:Engineering|role:Senior Developer|hire_date:2022-03-15")
(cd "$DEMO_DIR/employees" && "$PROLLY_BIN" set "employee:6002" "Bob Chen|department:Marketing|role:Marketing Manager|hire_date:2021-08-22")
(cd "$DEMO_DIR/employees" && "$PROLLY_BIN" set "employee:6003" "Carol Martinez|department:Sales|role:Sales Representative|hire_date:2023-01-10")
(cd "$DEMO_DIR/employees" && "$PROLLY_BIN" commit -m "Initial employee records")

(cd "$DEMO_DIR/employees" && "$PROLLY_BIN" set "employee:6004" "David Park|department:Engineering|role:DevOps Engineer|hire_date:2023-05-20")
(cd "$DEMO_DIR/employees" && "$PROLLY_BIN" set "employee:6005" "Emma Wilson|department:HR|role:HR Specialist|hire_date:2022-11-08")
(cd "$DEMO_DIR/employees" && "$PROLLY_BIN" set "employee:6006" "Frank Davis|department:Finance|role:Financial Analyst|hire_date:2023-02-14")
(cd "$DEMO_DIR/employees" && "$PROLLY_BIN" commit -m "Add new team members")

# Create HR management branch
(cd "$DEMO_DIR/employees" && git checkout -b hr/performance-reviews)
(cd "$DEMO_DIR/employees" && "$PROLLY_BIN" set "employee:6001" "Alice Johnson|department:Engineering|role:Senior Developer|hire_date:2022-03-15|performance:excellent")
(cd "$DEMO_DIR/employees" && "$PROLLY_BIN" set "employee:6002" "Bob Chen|department:Marketing|role:Marketing Manager|hire_date:2021-08-22|performance:good")
(cd "$DEMO_DIR/employees" && "$PROLLY_BIN" commit -m "Q2 2024 performance review updates")

(cd "$DEMO_DIR/employees" && "$PROLLY_BIN" set "employee:6007" "Grace Kim|department:Engineering|role:Junior Developer|hire_date:2024-06-01")
(cd "$DEMO_DIR/employees" && "$PROLLY_BIN" set "employee:6008" "Henry Lopez|department:Operations|role:Operations Manager|hire_date:2024-07-15")
(cd "$DEMO_DIR/employees" && "$PROLLY_BIN" commit -m "Summer 2024 new hires")

# Create payroll branch
(cd "$DEMO_DIR/employees" && git checkout -b payroll/salary-adjustments)
(cd "$DEMO_DIR/employees" && "$PROLLY_BIN" set "employee:6003" "Carol Martinez|department:Sales|role:Senior Sales Representative|hire_date:2023-01-10|promotion:2024-08-01")
(cd "$DEMO_DIR/employees" && "$PROLLY_BIN" set "employee:6004" "David Park|department:Engineering|role:Senior DevOps Engineer|hire_date:2023-05-20|promotion:2024-08-01")
(cd "$DEMO_DIR/employees" && "$PROLLY_BIN" commit -m "August 2024 promotions and salary adjustments")

# Back to main
(cd "$DEMO_DIR/employees" && git checkout main)

# Dataset 7: Analytics
echo "📈 Populating analytics dataset..."

# Main branch data
(cd "$DEMO_DIR/analytics" && "$PROLLY_BIN" set "metric:7001" "daily_sales|date:2024-08-01|value:15420.50|currency:USD")
(cd "$DEMO_DIR/analytics" && "$PROLLY_BIN" set "metric:7002" "daily_visitors|date:2024-08-01|value:1250|source:website")
(cd "$DEMO_DIR/analytics" && "$PROLLY_BIN" set "metric:7003" "conversion_rate|date:2024-08-01|value:3.2|unit:percent")
(cd "$DEMO_DIR/analytics" && "$PROLLY_BIN" commit -m "Initial analytics baseline - August 1")

(cd "$DEMO_DIR/analytics" && "$PROLLY_BIN" set "metric:7004" "daily_sales|date:2024-08-02|value:18750.25|currency:USD")
(cd "$DEMO_DIR/analytics" && "$PROLLY_BIN" set "metric:7005" "daily_visitors|date:2024-08-02|value:1420|source:website")
(cd "$DEMO_DIR/analytics" && "$PROLLY_BIN" set "metric:7006" "conversion_rate|date:2024-08-02|value:4.1|unit:percent")
(cd "$DEMO_DIR/analytics" && "$PROLLY_BIN" commit -m "August 2 analytics - improved performance")

(cd "$DEMO_DIR/analytics" && "$PROLLY_BIN" set "metric:7007" "weekly_retention|week:32|value:68.5|unit:percent|cohort:july")
(cd "$DEMO_DIR/analytics" && "$PROLLY_BIN" set "metric:7008" "customer_satisfaction|period:q2|value:4.3|scale:5.0|responses:245")
(cd "$DEMO_DIR/analytics" && "$PROLLY_BIN" commit -m "Weekly and quarterly metrics")

# Create reporting branch
(cd "$DEMO_DIR/analytics" && git checkout -b reporting/monthly-kpis)
(cd "$DEMO_DIR/analytics" && "$PROLLY_BIN" set "kpi:monthly_revenue" "month:july|value:487500.75|target:450000|variance:+8.3")
(cd "$DEMO_DIR/analytics" && "$PROLLY_BIN" set "kpi:customer_acquisition" "month:july|value:156|target:140|cost_per_acquisition:125.50")
(cd "$DEMO_DIR/analytics" && "$PROLLY_BIN" set "kpi:churn_rate" "month:july|value:2.1|target:2.5|unit:percent")
(cd "$DEMO_DIR/analytics" && "$PROLLY_BIN" commit -m "July 2024 monthly KPI report")

(cd "$DEMO_DIR/analytics" && "$PROLLY_BIN" set "forecast:august_revenue" "value:515000|confidence:85|model:linear_regression")
(cd "$DEMO_DIR/analytics" && "$PROLLY_BIN" set "forecast:q3_growth" "value:12.5|unit:percent|confidence:78|model:seasonal_arima")
(cd "$DEMO_DIR/analytics" && "$PROLLY_BIN" commit -m "Q3 revenue and growth forecasting")

# Create experiments branch
(cd "$DEMO_DIR/analytics" && git checkout -b experiments/ab-testing)
(cd "$DEMO_DIR/analytics" && "$PROLLY_BIN" set "experiment:homepage_cta" "variant_a:2.8|variant_b:4.2|significance:95|winner:b")
(cd "$DEMO_DIR/analytics" && "$PROLLY_BIN" set "experiment:checkout_flow" "variant_a:15.2|variant_b:13.8|significance:82|status:running")
(cd "$DEMO_DIR/analytics" && "$PROLLY_BIN" commit -m "A/B testing experiment results")

# Back to main
(cd "$DEMO_DIR/analytics" && git checkout main)

# Dataset 8: Reviews
echo "⭐ Populating reviews dataset..."

# Main branch data
(cd "$DEMO_DIR/reviews" && "$PROLLY_BIN" set "review:8001" "product:2001|customer:1001|rating:5|comment:Excellent laptop, very fast|date:2024-07-15")
(cd "$DEMO_DIR/reviews" && "$PROLLY_BIN" set "review:8002" "product:2002|customer:1002|rating:4|comment:Good mouse, comfortable grip|date:2024-07-18")
(cd "$DEMO_DIR/reviews" && "$PROLLY_BIN" set "review:8003" "product:2003|customer:1003|rating:5|comment:Love the mechanical feel|date:2024-07-20")
(cd "$DEMO_DIR/reviews" && "$PROLLY_BIN" commit -m "Initial product reviews")

(cd "$DEMO_DIR/reviews" && "$PROLLY_BIN" set "review:8004" "product:2001|customer:1004|rating:4|comment:Great performance, bit pricey|date:2024-07-25")
(cd "$DEMO_DIR/reviews" && "$PROLLY_BIN" set "review:8005" "product:2004|customer:1005|rating:3|comment:USB-C hub works but gets warm|date:2024-07-28")
(cd "$DEMO_DIR/reviews" && "$PROLLY_BIN" set "review:8006" "product:2002|customer:1006|rating:5|comment:Perfect wireless mouse|date:2024-08-01")
(cd "$DEMO_DIR/reviews" && "$PROLLY_BIN" commit -m "Additional customer feedback")

# Create moderation branch
(cd "$DEMO_DIR/reviews" && git checkout -b moderation/quality-check)
(cd "$DEMO_DIR/reviews" && "$PROLLY_BIN" set "review:8002" "product:2002|customer:1002|rating:4|comment:Good mouse, comfortable grip|date:2024-07-18|status:verified")
(cd "$DEMO_DIR/reviews" && "$PROLLY_BIN" set "review:8007" "product:2003|customer:1007|rating:1|comment:Spam content removed|date:2024-08-02|status:flagged|moderator_action:content_removed")
(cd "$DEMO_DIR/reviews" && "$PROLLY_BIN" commit -m "Review moderation and verification")

(cd "$DEMO_DIR/reviews" && "$PROLLY_BIN" set "review:8008" "product:2005|customer:1008|rating:4|comment:Solid monitor stand, easy setup|date:2024-08-05|status:verified")
(cd "$DEMO_DIR/reviews" && "$PROLLY_BIN" set "review:8009" "product:2006|customer:1009|rating:5|comment:Amazing sound quality for gaming|date:2024-08-06|status:verified")
(cd "$DEMO_DIR/reviews" && "$PROLLY_BIN" commit -m "New verified reviews")

# Create sentiment analysis branch
(cd "$DEMO_DIR/reviews" && git checkout -b analysis/sentiment-scoring)
(cd "$DEMO_DIR/reviews" && "$PROLLY_BIN" set "review:8001" "product:2001|customer:1001|rating:5|comment:Excellent laptop, very fast|date:2024-07-15|sentiment:positive|score:0.92")
(cd "$DEMO_DIR/reviews" && "$PROLLY_BIN" set "review:8004" "product:2001|customer:1004|rating:4|comment:Great performance, bit pricey|date:2024-07-25|sentiment:mixed|score:0.65")
(cd "$DEMO_DIR/reviews" && "$PROLLY_BIN" set "review:8005" "product:2004|customer:1005|rating:3|comment:USB-C hub works but gets warm|date:2024-07-28|sentiment:neutral|score:0.45")
(cd "$DEMO_DIR/reviews" && "$PROLLY_BIN" commit -m "Sentiment analysis integration")

# Back to main
(cd "$DEMO_DIR/reviews" && git checkout main)

# Dataset 9: Shipping
echo "🚚 Populating shipping dataset..."

# Main branch data
(cd "$DEMO_DIR/shipping" && "$PROLLY_BIN" set "shipment:9001" "order:3001|carrier:FedEx|tracking:1Z999AA1234567890|status:in_transit|origin:warehouse_a")
(cd "$DEMO_DIR/shipping" && "$PROLLY_BIN" set "shipment:9002" "order:3002|carrier:UPS|tracking:1Z999BB1234567890|status:delivered|origin:warehouse_a")
(cd "$DEMO_DIR/shipping" && "$PROLLY_BIN" set "shipment:9003" "order:3003|carrier:DHL|tracking:1Z999CC1234567890|status:out_for_delivery|origin:warehouse_b")
(cd "$DEMO_DIR/shipping" && "$PROLLY_BIN" commit -m "Initial shipping records")

(cd "$DEMO_DIR/shipping" && "$PROLLY_BIN" set "shipment:9004" "order:3004|carrier:USPS|tracking:1Z999DD1234567890|status:processing|origin:warehouse_a")
(cd "$DEMO_DIR/shipping" && "$PROLLY_BIN" set "shipment:9005" "order:3005|carrier:FedEx|tracking:1Z999EE1234567890|status:shipped|origin:warehouse_b")
(cd "$DEMO_DIR/shipping" && "$PROLLY_BIN" set "shipment:9006" "order:3006|carrier:UPS|tracking:1Z999FF1234567890|status:processing|origin:warehouse_c|priority:high")
(cd "$DEMO_DIR/shipping" && "$PROLLY_BIN" commit -m "Additional shipment tracking")

# Create logistics branch
(cd "$DEMO_DIR/shipping" && git checkout -b logistics/optimization)
(cd "$DEMO_DIR/shipping" && "$PROLLY_BIN" set "shipment:9001" "order:3001|carrier:FedEx|tracking:1Z999AA1234567890|status:delivered|origin:warehouse_a|delivery_date:2024-08-05|transit_days:3")
(cd "$DEMO_DIR/shipping" && "$PROLLY_BIN" set "shipment:9003" "order:3003|carrier:DHL|tracking:1Z999CC1234567890|status:delivered|origin:warehouse_b|delivery_date:2024-08-06|transit_days:2")
(cd "$DEMO_DIR/shipping" && "$PROLLY_BIN" commit -m "Update delivery status and transit times")

(cd "$DEMO_DIR/shipping" && "$PROLLY_BIN" set "route:optimization_a" "warehouse:a|destinations:5|total_distance:142.5|fuel_cost:89.25|driver:john_d")
(cd "$DEMO_DIR/shipping" && "$PROLLY_BIN" set "route:optimization_b" "warehouse:b|destinations:8|total_distance:201.3|fuel_cost:125.80|driver:sarah_m")
(cd "$DEMO_DIR/shipping" && "$PROLLY_BIN" commit -m "Route optimization data collection")

# Create returns branch
(cd "$DEMO_DIR/shipping" && git checkout -b returns/processing)
(cd "$DEMO_DIR/shipping" && "$PROLLY_BIN" set "return:9101" "original_order:3002|reason:defective|status:approved|return_carrier:UPS|tracking:1Z888BB1234567890")
(cd "$DEMO_DIR/shipping" && "$PROLLY_BIN" set "return:9102" "original_order:3001|reason:wrong_item|status:processing|expected_carrier:FedEx")
(cd "$DEMO_DIR/shipping" && "$PROLLY_BIN" commit -m "Customer return processing")

(cd "$DEMO_DIR/shipping" && "$PROLLY_BIN" set "return:9101" "original_order:3002|reason:defective|status:completed|return_carrier:UPS|tracking:1Z888BB1234567890|refund_issued:2024-08-08")
(cd "$DEMO_DIR/shipping" && "$PROLLY_BIN" set "return:9103" "original_order:3004|reason:customer_change_of_mind|status:rejected|policy_violation:return_window_expired")
(cd "$DEMO_DIR/shipping" && "$PROLLY_BIN" commit -m "Complete returns processing cycle")

# Create international branch
(cd "$DEMO_DIR/shipping" && git checkout -b international/customs)
(cd "$DEMO_DIR/shipping" && "$PROLLY_BIN" set "international:9201" "order:int_001|destination:Canada|customs_value:1250.00|duties:125.00|carrier:FedEx_International")
(cd "$DEMO_DIR/shipping" && "$PROLLY_BIN" set "international:9202" "order:int_002|destination:Germany|customs_value:890.50|duties:89.05|carrier:DHL_Express|status:customs_clearance")
(cd "$DEMO_DIR/shipping" && "$PROLLY_BIN" set "international:9203" "order:int_003|destination:Japan|customs_value:2100.75|duties:315.11|carrier:UPS_Worldwide|status:delivered")
(cd "$DEMO_DIR/shipping" && "$PROLLY_BIN" commit -m "International shipping and customs tracking")

# Back to main
(cd "$DEMO_DIR/shipping" && git checkout main)

## Generate the HTML output in a temporary directory
echo "📊 Generating unified multi-dataset visualization..."

# Create temporary directory for HTML output
TEMP_DIR=$(mktemp -d)
HTML_OUTPUT="$TEMP_DIR/prollytree-ui.html"

"$UI_BIN" "$DEMO_DIR" -o "$HTML_OUTPUT"

echo ""
echo "✅ HTML visualization generated successfully!"
echo "  📄 Output file: $HTML_OUTPUT"
echo "  🌐 Multi-dataset view with comprehensive git commit details"
echo "  📊 Features: Dataset switching, branch filtering, commit diff details"

cd "$PROJECT_ROOT"

# Open the HTML file from temp directory
echo "🌐 Opening visualization in browser..."
open "$HTML_OUTPUT"
