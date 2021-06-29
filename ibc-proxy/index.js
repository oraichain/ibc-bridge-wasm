const httpProxy = require('http-proxy');
const express = require('express');
const { hostMap } = require('./config.json');

const proxy = httpProxy.createProxyServer({ timeout: 10000 });
proxy.on('error', () => {
  // no handler
});
const app = express();

const port = process.env.PORT || 80;
app
  .all('*', (req, res) => {
    // cors processing
    res.header('Access-Control-Allow-Origin', '*');
    res.header(
      'Access-Control-Allow-Headers',
      'Content-Type,Content-Length, Authorization, Accept,X-Requested-With'
    );
    res.header('Access-Control-Allow-Methods', 'PUT,POST,GET,DELETE,OPTIONS');
    res.header('Access-Control-Allow-Credentials', true);
    res.setHeader('X-Powered-By', 'LCD API');
    if (req.method === 'OPTIONS') return res.sendStatus(200);

    const target = hostMap[req.headers.host];
    if (!target) return res.end();
    proxy.web(req, res, { target: `http://${target}` });
  })
  .listen(port, '0.0.0.0', () => {
    console.log(`listening on port ${port}`);
  })
  .on('upgrade', (req, socket, head) => {
    const target = hostMap[req.headers.host];
    if (!target) return socket.close();

    socket.on('error', () => {
      // no handler
    });
    proxy.ws(req, socket, head, { target: `ws://${target}` });
  });
