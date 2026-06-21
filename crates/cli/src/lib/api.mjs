import fs from 'fs';
import http from 'http';
import { spawn } from 'child_process';
import { pipeline } from 'stream/promises';
import { register } from 'node:module';

try {
    const loaderUrl = new URL('./loader.mjs', import.meta.url).href;
    register(loaderUrl);
} catch (err) {
    // Fail silently in older node versions
}

globalThis.Kumo = {
    version: "__KUMO_VERSION__",
    env: process.env,
    
    file: (path) => {
        return {
            text: async () => fs.promises.readFile(path, 'utf8'),
            json: async () => JSON.parse(await fs.promises.readFile(path, 'utf8')),
            exists: () => fs.existsSync(path),
        };
    },
    write: async (path, data) => {
        return fs.promises.writeFile(path, data);
    },
    
    spawn: (command, args = [], options = {}) => {
        return spawn(command, args, options);
    },
    sleep: (ms) => new Promise(r => setTimeout(r, ms)),
    
    serve: (options) => {
        const server = http.createServer(async (req, res) => {
            if (options.fetch) {
                try {
                    const protocol = req.socket.encrypted ? 'https' : 'http';
                    const url = new URL(req.url || '/', `${protocol}://${req.headers.host || 'localhost'}`);
                    
                    const init = {
                        method: req.method,
                        headers: req.headers,
                    };
                    
                    if (req.method !== 'GET' && req.method !== 'HEAD') {
                        const chunks = [];
                        for await (const chunk of req) {
                            chunks.push(chunk);
                        }
                        init.body = Buffer.concat(chunks);
                    }
                    
                    const request = new Request(url, init);
                    
                    const response = await options.fetch(request);
                    
                    response.headers.forEach((value, key) => {
                        res.setHeader(key, value);
                    });
                    res.writeHead(response.status || 200);
                    
                    if (response.body) {
                        for await (const chunk of response.body) {
                            res.write(chunk);
                        }
                        res.end();
                    } else {
                        res.end();
                    }
                } catch (err) {
                    console.error("Kumo server error:", err);
                    res.writeHead(500);
                    res.end("Internal Server Error");
                }
            } else {
                res.end();
            }
        });
        const port = options.port || 3000;
        server.listen(port, () => {
            console.log(`Kumo server running on port ${port}`);
        });
        return server;
    },
    
    pkg: {
        readConfig: async () => {
            if (fs.existsSync('kumo.json')) {
                return JSON.parse(await fs.promises.readFile('kumo.json', 'utf8'));
            }
            return null;
        }
    }
};
