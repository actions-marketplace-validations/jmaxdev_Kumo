interface KumoChildProcess {
    stdout: {
        on(event: 'data', listener: (chunk: Uint8Array) => void): void;
    } | null;
}

interface KumoAPI {
    version: string;
    env: Record<string, string | undefined>;
    file(path: string): {
        text(): Promise<string>;
        json<T = unknown>(): Promise<T>;
        exists(): boolean;
    };
    write(path: string, data: string | Uint8Array): Promise<void>;
    spawn(command: string, args?: string[], options?: object): KumoChildProcess;
    sleep(ms: number): Promise<void>;
    serve(options: { port?: number; fetch: (req: Request) => Promise<Response> | Response }): any;
    pkg: {
        readConfig(): Promise<unknown>;
    };
}

declare const Kumo: KumoAPI;

// Wildcard modules for URL imports (HTTPS module loader)
declare module "https://*" {
    const value: any;
    export default value;
}

declare module "http://*" {
    const value: any;
    export default value;
}

