// Capture Tenhou game protocol — waits through actual gameplay.
const { chromium } = require('playwright');

(async () => {
  const browser = await chromium.launch({ headless: true });
  const page = await browser.newPage();

  let messages = [];

  page.on('websocket', (ws) => {
    console.log(`\n*** WS: ${ws.url()} ***\n`);
    ws.on('framesent', f => { messages.push(`> ${f.payload}`); });
    ws.on('framereceived', f => {
      const text = f.payload.toString();
      console.log(`< ${text}`);
      messages.push(`< ${text}`);
    });
  });

  await page.goto('https://tenhou.net/3/', { waitUntil: 'domcontentloaded', timeout: 30000 });
  await page.waitForTimeout(2000);

  // Click OK
  const ok = await page.$('button:has-text("OK")');
  if (ok) { await ok.click(); await page.waitForTimeout(1000); }

  // Click Guest Login
  const guest = await page.$('button:has-text("Guest")');
  if (guest) { await guest.click(); await page.waitForTimeout(2000); }

  // Click first Join button
  const join = await page.$('button:has-text("Join")');
  if (join) {
    console.log('Clicking Join...');
    await join.click();
  }

  // Wait for game to start and play through (up to 5 minutes)
  console.log('Waiting for game messages (5 min max)...\n');
  for (let i = 0; i < 60; i++) {
    await page.waitForTimeout(5000);
    // Check if game ended
    const body = await page.evaluate(() => document.body?.innerText?.substring(0, 200));
    if (body.includes('LOGOUT')) {
      // Still in lobby/game — check if new game needed
      const joinBtn = await page.$('button:has-text("Join")');
      if (joinBtn && !body.includes('REJOIN')) {
        console.log('Lobby visible, clicking Join again...');
        await joinBtn.click();
      }
    }
  }

  // Save all messages
  const fs = require('fs');
  fs.writeFileSync('/tmp/tenhou_protocol.txt', messages.join('\n'));
  console.log(`\nSaved ${messages.length} messages to /tmp/tenhou_protocol.txt`);
  console.log(`Last 30 messages:`);
  messages.slice(-30).forEach(m => console.log(`  ${m.substring(0,300)}`));

  await browser.close();
})().catch(err => { console.error('FATAL:', err.message); process.exit(1); });
