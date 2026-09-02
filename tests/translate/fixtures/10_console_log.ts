// 10_console_log - various console.log with Js types, formatter test
let num: number = 42;
console.log(num);
console.log(123);

let str: string = "hello";
console.log(str);
console.log("world");

let flag: boolean = true;
console.log(flag);
console.log(false);

let arr: number[] = [1, 2, 3];
console.log(arr);
console.log(arr.length);

let obj = {a: 1, b: "test"};
console.log(obj);
console.log(obj["a"]);

let n1: number = 10;
let n2: number = 20;
console.log(n1, n2);
console.log("a", "b", "c");
console.log(num, str, flag);

function add(x: number, y: number): number {
    return x + y;
}
console.log(add(2, 5));
console.log(`Add is ${add(3, 4)}`);
console.log(`This 2 + 5 = ${add(2, 5)}`);

let s1: string = "Hello";
let s2: string = "World";
console.log(`${s1}, ${s2}!`);
console.log(`Number is ${num}, string is ${str}`);

console.log(`Multiple ${num} and ${str} and ${flag}`);

let a: string = "Hello";
let b: string = "World";
let c: string = `${a}, ${b}!`;
console.log(c);

console.log("plain string");
console.log(`plain template`);
console.log(`Value: ${42}`);
console.log(`Flag: ${true}`);
