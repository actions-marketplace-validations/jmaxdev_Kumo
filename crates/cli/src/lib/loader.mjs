import fs from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

let configLoaded = false;
let allowedImportHosts = [];

async function loadConfig() {
  if (configLoaded) return;
  configLoaded = true;

  // Try JSON first
  try {
    const jsonPath = path.join(process.cwd(), 'kumo.config.json');
    if (fs.existsSync(jsonPath)) {
      const data = JSON.parse(fs.readFileSync(jsonPath, 'utf8'));
      const hosts = data.AllowedImportHost || data.allowedImportHosts || data.AllowedImportHosts || data.allowed_import_hosts;
      if (Array.isArray(hosts)) {
        allowedImportHosts = hosts.map(h => h.toLowerCase().trim());
        return;
      }
    }
  } catch (e) { }

  // Try JS next
  try {
    const jsPath = path.join(process.cwd(), 'kumo.config.js');
    if (fs.existsSync(jsPath)) {
      const moduleUrl = pathToFileURL(jsPath).href;
      const mod = await import(moduleUrl);
      const data = mod.default || mod;
      const hosts = data.AllowedImportHost || data.allowedImportHosts || data.AllowedImportHosts || data.allowed_import_hosts;
      if (Array.isArray(hosts)) {
        allowedImportHosts = hosts.map(h => h.toLowerCase().trim());
        return;
      }
    }
  } catch (e) { }
}

function reportErrorAndExit(err) {
  const msg = err.message;
  if (msg.startsWith('Security restriction:')) {
    const details = msg.slice('Security restriction:'.length).trim();
    fs.writeSync(2, `\x1b[31m[Error] Security restriction: ${details}\x1b[0m\n`);
  } else {
    fs.writeSync(2, `\x1b[31m[Error] ${msg}\x1b[0m\n`);
  }
  process.exit(1);
}

function validateUrl(urlString, allowedHosts) {
  if (urlString.startsWith('http://')) {
    throw new Error(`Security restriction: HTTP imports are not allowed (${urlString}). Only HTTPS is supported.`);
  }
  if (urlString.startsWith('https://')) {
    const parsed = new URL(urlString);
    const hostname = parsed.hostname.toLowerCase();
    if (
      hostname === 'localhost' ||
      hostname === '127.0.0.1' ||
      hostname === '[::1]' ||
      hostname === '0.0.0.0'
    ) {
      throw new Error(`Security restriction: HTTPS imports from localhost or loopback addresses are not allowed (${urlString}).`);
    }

    if (allowedHosts.length === 0) {
      throw new Error(`Security restriction: All URL imports are blocked by default. If you want to import from this host (${hostname}), add it to the 'allowedImportHosts' list in your configuration kumo.config.json`);
    }

    const isAllowed = allowedHosts.some(allowedHost => {
      return hostname === allowedHost || hostname.endsWith('.' + allowedHost);
    });

    if (!isAllowed) {
      throw new Error(`Security restriction: Host '${hostname}' is not allowed for HTTPS imports. If you want to import this script, add the host to the allowedImportHosts list in your configuration.`);
    }
  }
}

export async function resolve(specifier, context, nextResolve) {
  try {
    await loadConfig();
    validateUrl(specifier, allowedImportHosts);
  } catch (err) {
    reportErrorAndExit(err);
  }

  if (specifier.startsWith('https://')) {
    return {
      shortCircuit: true,
      url: specifier
    };
  }

  if (context.parentURL && context.parentURL.startsWith('https://')) {
    if (specifier.startsWith('.') || specifier.startsWith('/')) {
      const resolved = new URL(specifier, context.parentURL).href;
      try {
        validateUrl(resolved, allowedImportHosts);
      } catch (err) {
        reportErrorAndExit(err);
      }
      return {
        shortCircuit: true,
        url: resolved
      };
    } else {
      const localParentURL = pathToFileURL(process.cwd() + '/index.js').href;
      return nextResolve(specifier, {
        ...context,
        parentURL: localParentURL
      });
    }
  }

  return nextResolve(specifier, context);
}

export async function load(url, context, nextLoad) {
  try {
    await loadConfig();
    validateUrl(url, allowedImportHosts);
  } catch (err) {
    reportErrorAndExit(err);
  }

  if (url.startsWith('https://')) {
    let res;
    try {
      res = await fetch(url);
      if (!res.ok) {
        throw new Error(`Failed to fetch ${url}: ${res.statusText}`);
      }
    } catch (err) {
      reportErrorAndExit(err);
    }
    const source = await res.text();

    let format = 'module';
    if (url.endsWith('.json')) {
      format = 'json';
    }

    return {
      shortCircuit: true,
      format,
      source
    };
  }

  return nextLoad(url, context);
}
