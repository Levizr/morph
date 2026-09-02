// 14_inheritance - test class inheritance, super, interfaces, enums
class Shape {
    name: string;
    constructor(name: string) {
        this.name = name;
    }
    area(): number {
        return 0;
    }
    describe(): string {
        return `Shape: ${this.name}`;
    }
}

class Circle extends Shape {
    radius: number;
    constructor(radius: number) {
        super("Circle");
        this.radius = radius;
    }
    area(): number {
        return this.radius * this.radius * 3;
    }
    describe(): string {
        return `${this.name} with radius ${this.radius}`;
    }
}

let c = new Circle(5);
console.log(c.describe());
console.log(c.area());
console.log(c.name);
console.log(c.radius);

class Rectangle extends Shape {
    width: number;
    height: number;
    constructor(width: number, height: number) {
        super("Rectangle");
        this.width = width;
        this.height = height;
    }
    area(): number {
        return this.width * this.height;
    }
}
let r = new Rectangle(4, 5);
console.log(r.area());
console.log(r.describe());

interface Printable {
    print(): string;
}
class Document implements Printable {
    content: string;
    constructor(content: string) {
        this.content = content;
    }
    print(): string {
        return `Doc: ${this.content}`;
    }
}
let doc = new Document("Hello");
console.log(doc.print());

enum Color {
    Red,
    Green,
    Blue
}
let col: Color = Color.Red;
console.log(col === Color.Red);

class Counter {
    private count: number = 0;
    public increment(): void {
        this.count++;
    }
    public getCount(): number {
        return this.count;
    }
}
let counter = new Counter();
counter.increment();
counter.increment();
console.log(counter.getCount());

class WithStatic {
    static version: string = "1.0";
    static getVersion(): string {
        return WithStatic.version;
    }
}
console.log(WithStatic.getVersion());
console.log(WithStatic.version);
