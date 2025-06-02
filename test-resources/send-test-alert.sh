#!/bin/bash
# Script to send test alerts to Punching Fist webhook endpoint

# Set the operator URL (defaults to localhost with port forwarding)
OPERATOR_URL="${OPERATOR_URL:-http://localhost:8080}"

# Function to send a test alert
send_alert() {
    local alertname=$1
    local severity=$2
    local pod=$3
    local description=$4
    
    echo "Sending $alertname alert for pod $pod..."
    
    curl -X POST "${OPERATOR_URL}/webhook/test-alerts" \
        -H "Content-Type: application/json" \
        -d @- <<EOF
{
  "version": "4",
  "groupKey": "{}:{alertname=\"${alertname}\"}",
  "truncatedAlerts": 0,
  "status": "firing",
  "receiver": "punchingfist",
  "groupLabels": {
    "alertname": "${alertname}"
  },
  "commonLabels": {
    "alertname": "${alertname}",
    "severity": "${severity}"
  },
  "commonAnnotations": {
    "description": "${description}"
  },
  "externalURL": "http://alertmanager:9093",
  "alerts": [
    {
      "status": "firing",
      "labels": {
        "alertname": "${alertname}",
        "severity": "${severity}",
        "namespace": "test-workloads",
        "pod": "${pod}",
        "container": "app"
      },
      "annotations": {
        "description": "${description}",
        "summary": "Test alert for ${alertname}"
      },
      "startsAt": "$(date -u +%Y-%m-%dT%H:%M:%SZ)",
      "endsAt": "0001-01-01T00:00:00Z",
      "generatorURL": "http://prometheus:9090/alerts",
      "fingerprint": "$(echo -n "${alertname}${pod}" | shasum -a 256 | cut -d' ' -f1 | cut -c1-16)"
    }
  ]
}
EOF
    
    echo -e "\n✅ Alert sent!\n"
    sleep 2
}

# Main menu
echo "🤖 Punching Fist Test Alert Generator"
echo "===================================="
echo "Which test alert would you like to send?"
echo ""
echo "1) TestPodCrashLooping - Test crash loop investigation"
echo "2) TestPodHighCPUUsage - Test high CPU investigation"
echo "3) TestPodHighMemoryUsage - Test high memory investigation"
echo "4) Send all test alerts"
echo "5) Custom alert"
echo ""
read -p "Enter your choice (1-5): " choice

case $choice in
    1)
        send_alert "TestPodCrashLooping" "critical" "crashloop-app" \
            "Pod crashloop-app in namespace test-workloads has been restarting frequently"
        ;;
    2)
        send_alert "TestPodHighCPUUsage" "warning" "cpu-intensive" \
            "Pod cpu-intensive is using excessive CPU resources"
        ;;
    3)
        send_alert "TestPodHighMemoryUsage" "warning" "memory-hog" \
            "Pod memory-hog is using excessive memory resources"
        ;;
    4)
        echo "Sending all test alerts..."
        send_alert "TestPodCrashLooping" "critical" "crashloop-app" \
            "Pod crashloop-app in namespace test-workloads has been restarting frequently"
        send_alert "TestPodHighCPUUsage" "warning" "cpu-intensive" \
            "Pod cpu-intensive is using excessive CPU resources"
        send_alert "TestPodHighMemoryUsage" "warning" "memory-hog" \
            "Pod memory-hog is using excessive memory resources"
        ;;
    5)
        echo ""
        echo "⚠️  Note: The webhook is configured with filters that only accept:"
        echo "   - Alert names: TestPodCrashLooping, TestPodHighCPUUsage, TestPodHighMemoryUsage, TestPodNotReady"
        echo "   - Severities: critical, warning"
        echo ""
        echo "Custom alerts that don't match these filters will be rejected."
        echo ""
        read -p "Enter alert name: " custom_alert
        read -p "Enter severity (critical/warning): " custom_severity
        read -p "Enter pod name: " custom_pod
        read -p "Enter description: " custom_description
        send_alert "$custom_alert" "$custom_severity" "$custom_pod" "$custom_description"
        ;;
    *)
        echo "Invalid choice!"
        exit 1
        ;;
esac

echo "✨ Done! Check the operator logs to see the investigation results:"
echo "   kubectl logs -n punching-fist -l app=punching-fist -f" 