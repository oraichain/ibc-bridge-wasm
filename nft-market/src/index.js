import React from 'react';
import ReactDOM from 'react-dom';
import './index.css';
import App from './App';
import Header from './Header';
import Wasm from './wasm';
const wasm = new Wasm('mars');
window.wasm = wasm;

ReactDOM.render(
  <>
    <Header />
    <App />
  </>,
  document.getElementById('root')
);
