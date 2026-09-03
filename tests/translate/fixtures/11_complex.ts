// 11_complex - combined complex example with all features
class User {
    name: string;
    age: number;
    hobbies: string[];
    constructor(name: string, age: number, hobbies: string[]) {
        this.name = name;
        this.age = age;
        this.hobbies = hobbies;
    }
    greet(): string {
        return `Hi, I'm ${this.name}, ${this.age} years old`;
    }
    addHobby(hobby: string): void {
        this.hobbies.push(hobby);
    }
    getHobbyCount(): number {
        return this.hobbies.length;
    }
}

let user = new User("Alice", 30, ["reading", "coding"]);
console.log(user.greet());
console.log(user.getHobbyCount());
user.addHobby("gaming");
console.log(user.getHobbyCount());
console.log(user.hobbies.length);
console.log(user.hobbies[0]);

function processUsers(users: number[]): number {
    let totalAge: number = 0;
    for (let i: number = 0; i < users.length; i++) {
        totalAge = totalAge + (users[i] as number);
    }
    return totalAge;
}

let users: number[] = [1, 2, 3];
console.log(users.length);
console.log(processUsers(users));

let data = {
    count: users.length,
    title: "User List"
};
console.log(data["count"]);
console.log(data["title"]);

let numbers: number[] = [1, 2, 3, 4, 5];
let sum: number = 0;
for (let i: number = 0; i < numbers.length; i++) {
    sum = sum + numbers[i];
}
console.log(sum);

let filtered: number[] = [];
for (let i: number = 0; i < numbers.length; i++) {
    let n: number = numbers[i];
    if (n % 2 === 1) {
        filtered.push(n);
    }
}
console.log(filtered.length);
console.log(filtered[0]);
console.log(filtered[1]);

let config = {threshold: 10, enabled: true};
let value: number = 15;
let result: string = value > config["threshold"] && config["enabled"] ? "pass" : "fail";
console.log(result);

let maybe: any = null;
let fallback: any = maybe ?? "default";
console.log(fallback);

let msg: string = `User ${user.name} has ${user.getHobbyCount()} hobbies`;
console.log(msg);

function testTry(): void {
    try {
        let risky: any = user.hobbies[10];
        if (risky === undefined) throw new Error("out of bounds");
        console.log(risky);
    } catch (e) {
        console.log("caught");
    } finally {
        console.log("done");
    }
}
testTry();

function factorial(n: number): number {
    if (n <= 1) return 1;
    return n * factorial(n - 1);
}
console.log(factorial(5));

let arr2: number[] = [10, 20, 30];
let first = arr2[0];
let second = arr2[1];
console.log(first);
console.log(second);

let combined: string = `${user.name} - ${user.hobbies.length} hobbies - sum ${sum}`;
console.log(combined);
