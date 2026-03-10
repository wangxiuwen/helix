$ErrorActionPreference = "Continue"
$baseUrl = "http://localhost:9520/api/agent/chat"
$accountId = "stress-test-100"
$totalRounds = 100
$successCount = 0
$failCount = 0

$prompts = @(
    "Describe the history of Linux in 200 words",
    "Explain Docker container technology in 300 words with examples",
    "Describe the architecture of Kubernetes and its main components in 200 words",
    "Analyze the pros and cons of microservices architecture in 300 words",
    "Describe CI/CD best practices for modern software teams in 200 words",
    "Explain cloud native technology stack and its evolution in 300 words",
    "List and explain 10 most commonly used Git commands with use cases",
    "Describe Nginx reverse proxy and load balancing configuration in 200 words",
    "Explain Prometheus and Grafana monitoring setup in 300 words",
    "Describe the five data structures of Redis and their use cases in 200 words"
)

Write-Host "========================================"
Write-Host "  Helix Context Optimization Stress Test"
Write-Host "  Target: $totalRounds rounds"
Write-Host "  Account: $accountId"
Write-Host "========================================"
Write-Host ""

$startTime = Get-Date

for ($i = 1; $i -le $totalRounds; $i++) {
    $prompt = $prompts[($i - 1) % $prompts.Count]
    $bodyObj = @{
        account_id = $accountId
        message    = "[Round $i/$totalRounds] $prompt"
    }
    $body = $bodyObj | ConvertTo-Json -Compress

    $roundStart = Get-Date
    Write-Host "[$i/$totalRounds] Sending..." -NoNewline

    try {
        $response = Invoke-RestMethod -Uri $baseUrl -Method POST -ContentType 'application/json' -Body $body -TimeoutSec 120
        $elapsed = [math]::Round(((Get-Date) - $roundStart).TotalSeconds, 1)
        $replyLen = 0
        if ($response.reply) { $replyLen = $response.reply.Length }

        if ($response.error) {
            Write-Host " ERROR (${elapsed}s): $($response.error)"
            $failCount++
        }
        else {
            Write-Host " OK (${elapsed}s, reply=${replyLen} chars)"
            $successCount++
        }
    }
    catch {
        $elapsed = [math]::Round(((Get-Date) - $roundStart).TotalSeconds, 1)
        Write-Host " FAILED (${elapsed}s): $_"
        $failCount++
    }
}

$totalElapsed = [math]::Round(((Get-Date) - $startTime).TotalSeconds, 1)

Write-Host ""
Write-Host "========================================"
Write-Host "  Test Complete!"
Write-Host "  Total time: ${totalElapsed}s"
Write-Host "  Success: $successCount / $totalRounds"
Write-Host "  Failed:  $failCount / $totalRounds"
Write-Host "========================================"

Write-Host ""
Write-Host "Checking logs for context truncation triggers..."

$logDate = Get-Date -Format 'yyyy-MM-dd'
$logFile = "$env:APPDATA\helix\logs\app.log.$logDate"
if (Test-Path $logFile) {
    $truncations = Select-String -Path $logFile -Pattern "optimize_chat_history_values triggered" -SimpleMatch
    if ($truncations) {
        Write-Host "  [PASS] optimize_chat_history_values was triggered $($truncations.Count) time(s)!"
    }
    else {
        Write-Host "  [INFO] optimize_chat_history_values was NOT triggered (messages within limit)"
    }

    $agentLogs = Select-String -Path $logFile -Pattern "stress-test-100" -SimpleMatch
    if ($agentLogs) {
        Write-Host "  [INFO] Agent processed $($agentLogs.Count) messages for stress-test account"
    }
}
else {
    Write-Host "  [WARN] Log file not found: $logFile"
}
