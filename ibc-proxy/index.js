const httpProxy = require('http-proxy');
const express = require('express');
const path = require('path');
const getAccount = require('./get-account');
const cors = require('cors');
const { hostMap } = require('./config.json');

const proxy = httpProxy.createProxyServer({ timeout: 10000 });
proxy.on('error', () => {
  // no handler
});
const app = express();

const port = process.env.PORT || 80;
app
  .use(cors())  
  .all('*', (req, res, next) => {
    const target = hostMap[req.headers.host.split(':')[0]];
    if (!target) return next();
    proxy.web(req, res, { target: `http://${target}` });
  })
  .use('/', express.static('frontend'))
  .get('/accounts', async (req, res) => {
    const accounts = await getAccount('accounts');
    res.json(accounts);
  })
  .use('/swagger', express.static('swagger-ui'))
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
