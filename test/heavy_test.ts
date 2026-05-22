// heavy_test.ts
// A fully typed, complex in-memory database engine with generic query builders,
// advanced type inference, mapped types, and deep nesting to stress test `tsc`.

type Primitive = string | number | boolean | null | Date;

// Deeply nested type resolution to stress the compiler
type DeepPartial<T> = T extends Function
    ? T
    : T extends Array<infer U>
    ? _DeepPartialArray<U>
    : T extends object
    ? _DeepPartialObject<T>
    : T | undefined;
interface _DeepPartialArray<T> extends Array<DeepPartial<T>> {}
type _DeepPartialObject<T> = { [P in keyof T]?: DeepPartial<T[P]> };

interface TableSchema {
    [column: string]: Primitive;
}

type QueryOperators<T> = {
    $eq?: T;
    $neq?: T;
    $gt?: T;
    $gte?: T;
    $lt?: T;
    $lte?: T;
    $in?: T[];
    $nin?: T[];
    $like?: string;
};

type WhereClause<T extends TableSchema> = {
    [K in keyof T]?: T[K] | QueryOperators<T[K]>;
} & {
    $and?: WhereClause<T>[];
    $or?: WhereClause<T>[];
};

type OrderBy<T extends TableSchema> = {
    [K in keyof T]?: 'ASC' | 'DESC';
};

interface Query<T extends TableSchema> {
    where?: WhereClause<T>;
    orderBy?: OrderBy<T>;
    limit?: number;
    offset?: number;
}

// Complex Event System
type EventCallback<T = any> = (data: T) => void | Promise<void>;

class EventEmitter<Events extends Record<string, any>> {
    private listeners: { [K in keyof Events]?: EventCallback<Events[K]>[] } = {};

    on<K extends keyof Events>(event: K, callback: EventCallback<Events[K]>): void {
        if (!this.listeners[event]) {
            this.listeners[event] = [];
        }
        this.listeners[event]!.push(callback);
    }

    emit<K extends keyof Events>(event: K, data: Events[K]): void {
        const eventListeners = this.listeners[event];
        if (eventListeners) {
            for (const listener of eventListeners) {
                listener(data);
            }
        }
    }
}

// Database implementation
export class InMemoryTable<T extends TableSchema> extends EventEmitter<{
    insert: T;
    update: { old: T; new: T };
    delete: T;
}> {
    private data: T[] = [];
    private idCounter = 1;

    constructor(public readonly name: string) {
        super();
    }

    public insert(record: Omit<T, 'id'>): T {
        const newRecord = { ...record, id: this.idCounter++ } as unknown as T;
        this.data.push(newRecord);
        this.emit('insert', newRecord);
        return newRecord;
    }

    public insertMany(records: Omit<T, 'id'>[]): T[] {
        return records.map((r) => this.insert(r));
    }

    public find(query: Query<T> = {}): T[] {
        let results = this.data.filter((row) => this.matchWhere(row, query.where));

        if (query.orderBy) {
            results = this.sortResults(results, query.orderBy);
        }

        if (query.offset !== undefined) {
            results = results.slice(query.offset);
        }

        if (query.limit !== undefined) {
            results = results.slice(0, query.limit);
        }

        return results;
    }

    public findOne(query: Query<T>): T | null {
        const results = this.find({ ...query, limit: 1 });
        return results.length > 0 ? results[0] : null;
    }

    public update(where: WhereClause<T>, partial: DeepPartial<T>): number {
        let count = 0;
        for (let i = 0; i < this.data.length; i++) {
            if (this.matchWhere(this.data[i], where)) {
                const oldRecord = { ...this.data[i] };
                this.data[i] = this.deepMerge(this.data[i], partial);
                this.emit('update', { old: oldRecord, new: this.data[i] });
                count++;
            }
        }
        return count;
    }

    public delete(where: WhereClause<T>): number {
        const initialLength = this.data.length;
        const deleted = this.data.filter((row) => this.matchWhere(row, where));
        this.data = this.data.filter((row) => !this.matchWhere(row, where));
        
        for (const row of deleted) {
            this.emit('delete', row);
        }
        
        return initialLength - this.data.length;
    }

    private matchWhere(row: T, where?: WhereClause<T>): boolean {
        if (!where) return true;

        if (where.$and) {
            if (!where.$and.every((w) => this.matchWhere(row, w))) return false;
        }

        if (where.$or) {
            if (!where.$or.some((w) => this.matchWhere(row, w))) return false;
        }

        for (const key in where) {
            if (key === '$and' || key === '$or') continue;

            const condition = where[key as keyof WhereClause<T>];
            const value = row[key];

            if (condition !== null && typeof condition === 'object' && !Array.isArray(condition) && !(condition instanceof Date)) {
                const ops = condition as QueryOperators<any>;
                if (ops.$eq !== undefined && value !== ops.$eq) return false;
                if (ops.$neq !== undefined && value === ops.$neq) return false;
                if (ops.$gt !== undefined && value <= ops.$gt) return false;
                if (ops.$gte !== undefined && value < ops.$gte) return false;
                if (ops.$lt !== undefined && value >= ops.$lt) return false;
                if (ops.$lte !== undefined && value > ops.$lte) return false;
                if (ops.$in !== undefined && !ops.$in.includes(value)) return false;
                if (ops.$nin !== undefined && ops.$nin.includes(value)) return false;
                if (ops.$like !== undefined && typeof value === 'string') {
                    const regex = new RegExp(ops.$like.replace(/%/g, '.*'), 'i');
                    if (!regex.test(value)) return false;
                }
            } else {
                if (value !== condition) return false;
            }
        }

        return true;
    }

    private sortResults(results: T[], orderBy: OrderBy<T>): T[] {
        return [...results].sort((a, b) => {
            for (const key in orderBy) {
                const direction = orderBy[key] === 'ASC' ? 1 : -1;
                if (a[key] < b[key]) return -1 * direction;
                if (a[key] > b[key]) return 1 * direction;
            }
            return 0;
        });
    }

    private deepMerge(target: any, source: any): any {
        if (typeof target !== 'object' || target === null) return source;
        if (typeof source !== 'object' || source === null) return source;

        const output = { ...target };
        for (const key in source) {
            if (source.hasOwnProperty(key)) {
                if (typeof source[key] === 'object' && source[key] !== null && !Array.isArray(source[key]) && !(source[key] instanceof Date)) {
                    output[key] = this.deepMerge(target[key], source[key]);
                } else {
                    output[key] = source[key];
                }
            }
        }
        return output;
    }
}

// --- DEMO / USAGE ---

interface User extends TableSchema {
    id: number;
    username: string;
    email: string;
    age: number;
    isActive: boolean;
    createdAt: Date;
}

const users = new InMemoryTable<User>('users');

users.on('insert', (user) => console.log(`New user created: ${user.username}`));
users.on('update', ({ old, new: updated }) => console.log(`User ${old.username} updated age from ${old.age} to ${updated.age}`));

// Insert a large batch of data to test performance
const fakeUsers: Omit<User, 'id'>[] = Array.from({ length: 1000 }).map((_, i) => ({
    username: `user_${i}`,
    email: `user_${i}@example.com`,
    age: 18 + (i % 50),
    isActive: i % 2 === 0,
    createdAt: new Date(),
}));

users.insertMany(fakeUsers);

// Complex Query 1: Find active users over 30, sorted by age DESC
const activeOlderUsers = users.find({
    where: {
        isActive: true,
        age: { $gt: 30, $lt: 50 },
    },
    orderBy: {
        age: 'DESC',
    },
    limit: 10,
});

console.log('Active older users:', activeOlderUsers.length);

// Complex Query 2: Deep nested OR/AND conditions
const specificUsers = users.find({
    where: {
        $or: [
            { username: { $like: 'user_1%' } },
            { 
                $and: [
                    { isActive: false },
                    { age: { $in: [20, 25, 30] } }
                ]
            }
        ]
    }
});

console.log('Specific users found:', specificUsers.length);

// Update operation
users.update(
    { age: { $lt: 20 } },
    { isActive: false }
);

// Delete operation
const deletedCount = users.delete({ isActive: false });
console.log('Deleted inactive users:', deletedCount);

// Verify types are strictly checked:
// users.insert({ username: 'test', age: 'should-fail', email: '...', isActive: true, createdAt: new Date() }); // TS Error if uncommented
