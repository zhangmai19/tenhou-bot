// Try connecting to Tenhou WebSocket servers at various known addresses.
const WebSocket = require('ws');

// Known Tenhou game server patterns
const targets = [
  // Old TCP game server (might have WSS on different port)
  { host: '133.242.10.78', port: 57883, tls: true },
  { host: '133.242.10.78', port: 57884, tls: true },
  { host: '133.242.10.78', port: 443, tls: true },
  { host: '133.242.10.78', port: 80, tls: false },
  // Main domain
  { host: 'tenhou.net', port: 443, tls: true },
  { host: 'tenhou.net', port: 57883, tls: true },
  // Common subdomains for game servers
  { host: 'w0.tenhou.net', port: 57883, tls: true },
  { host: 'w0.tenhou.net', port: 443, tls: true },
  { host: 'w1.tenhou.net', port: 57883, tls: true },
  { host: 'w2.tenhou.net', port: 57883, tls: true },
  { host: 'game.tenhou.net', port: 57883, tls: true },
  { host: 'game.tenhou.net', port: 443, tls: true },
];

async function tryConnect(target) {
  const scheme = target.tls ? 'wss' : 'ws';
  const url = `${scheme}://${target.host}:${target.port}/`;

  return new Promise((resolve) => {
    console.log(`Trying ${url}...`);
    const ws = new WebSocket(url, {
      rejectUnauthorized: false,
      timeout: 5000,
    });

    const timer = setTimeout(() => {
      ws.terminate();
      console.log(`  ${url} → TIMEOUT`);
      resolve(null);
    }, 5000);

    ws.on('open', () => {
      clearTimeout(timer);
      console.log(`  ${url} → CONNECTED!`);
      // Send HELO to see if it's a game server
      ws.send('<HELO name="NoName" tid="f0" sx="M" />\0');
    });

    ws.on('message', (data) => {
      console.log(`  < ${data.toString().trim()}`);
    });

    ws.on('error', (err) => {
      clearTimeout(timer);
      console.log(`  ${url} → ERROR: ${err.message}`);
      resolve(null);
    });

    // After 3 seconds with data, consider it success
    setTimeout(() => {
      if (ws.readyState === WebSocket.OPEN) {
        console.log(`  ${url} → SUCCESS (receiving data)`);
        ws.close();
        resolve(url);
      } else {
        resolve(null);
      }
    }, 3000);
  });
}

async function main() {
  console.log('Scanning for Tenhou WebSocket servers...\n');

  for (const target of targets) {
    const result = await tryConnect(target);
    if (result) {
      console.log(`\n*** FOUND: ${result} ***\n`);
    }
  }

  console.log('\nDone scanning.');
}

main().catch(console.error);
