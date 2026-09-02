// 03_classes - basic class, constructor, methods, static, inheritance
class Point {
    x: number;
    y: number;
    constructor(x: number, y: number) {
        this.x = x;
        this.y = y;
    }
    toString(): string {
        return `Point(${this.x}, ${this.y})`;
    }
    distance(): number {
        return this.x * this.x + this.y * this.y;
    }
}
let p = new Point(3, 4);
console.log(p.toString());
console.log(p.distance());

class Counter {
    count: number = 0;
    increment(): void {
        this.count++;
    }
    getCount(): number {
        return this.count;
    }
}
let c = new Counter();
c.increment();
c.increment();
console.log(c.getCount());

class Animal {
    name: string;
    constructor(name: string) {
        this.name = name;
    }
    speak(): string {
        return `${this.name} makes a noise`;
    }
}
class Dog extends Animal {
    breed: string;
    constructor(name: string, breed: string) {
        super(name);
        this.breed = breed;
    }
    speak(): string {
        return `${this.name} barks`;
    }
}
let dog = new Dog("Rex", "Labrador");
console.log(dog.speak());
console.log(dog.name);
console.log(dog.breed);

class Calculator {
    static add(a: number, b: number): number {
        return a + b;
    }
}
console.log(Calculator.add(10, 20));

class WithPrivate {
    private secret: number = 42;
    getSecret(): number {
        return this.secret;
    }
}
let wp = new WithPrivate();
console.log(wp.getSecret());
