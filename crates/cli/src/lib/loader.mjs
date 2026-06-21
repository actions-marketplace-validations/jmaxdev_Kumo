import { pathToFileURL } from 'node:url';

function validateUrl(urlString) {
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
  }
}

export async function resolve(specifier, context, nextResolve) {
  validateUrl(specifier);

  if (specifier.startsWith('https://')) {
    return {
      shortCircuit: true,
      url: specifier
    };
  }
  
  if (context.parentURL && context.parentURL.startsWith('https://')) {
    if (specifier.startsWith('.') || specifier.startsWith('/')) {
      const resolved = new URL(specifier, context.parentURL).href;
      validateUrl(resolved);
      return {
        shortCircuit: true,
        url: resolved
      };
    } else {
      // Resolve bare imports relative to the local project CWD so they are loaded from local node_modules
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
  validateUrl(url);

  if (url.startsWith('https://')) {
    const res = await fetch(url);
    if (!res.ok) {
      throw new Error(`Failed to fetch ${url}: ${res.statusText}`);
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
