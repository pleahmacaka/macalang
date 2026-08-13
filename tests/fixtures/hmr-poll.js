const urls = [];
const replies = ["{\"gen\":20}",
                 "{\"gen\":21,\"ops\":[{\"key\":\"k\",\"value\":\"v\"}]}",
                 "{\"reload\":true}"];
let applied = null, reloaded = false;
global.window = global;
global.location = { reload: () => { reloaded = true; } };
global.document = { readyState: "complete", addEventListener: () => {} };
global.macaSignal = ops => { applied = ops; };
global.setTimeout = f => { if (urls.length < replies.length) f(); };
global.XMLHttpRequest = function () {
  this.open = (m, u) => { urls.push(u); this.i = urls.length - 1; };
  this.send = () => { this.responseText = replies[this.i]; this.onload(); };
};
__POLLER__
console.log(JSON.stringify({ urls, applied, reloaded }));
