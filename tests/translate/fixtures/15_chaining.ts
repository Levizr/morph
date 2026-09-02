// 15_chaining - test method chaining
let s: string = "hello world";
console.log(s.split(" ").length);
console.log(s.toUpperCase().toLowerCase());
console.log(s.trim().toUpperCase());
console.log(s.substring(0, 5).toUpperCase());
console.log(s.slice(0, 5).length);

let num: number = 1234;
console.log(num.toString().charAt(0));
console.log(num.toString().charAt(0).toUpperCase());
console.log(num.toString().slice(1));
console.log(num.toString().length);

let str: string = "  Hello World  ";
console.log(str.trim().toUpperCase().split(" ").length);
console.log(str.trim().split(" ")[0]);

let arr: string[] = ["a", "b", "c"];
console.log(arr.length);
console.log(arr[0].toUpperCase());

let obj = {name: "test", value: "hello"};
console.log(obj["name"].toUpperCase());
console.log(obj["value"].split(" ").length);

// Chaining with array and string
let text: string = "a,b,c";
console.log(text.split(",").length);
console.log(text.split(",")[1]);
console.log(text.split(",")[1].toUpperCase());

// Multiple chaining
let n: number = 42;
console.log(n.toString().charAt(0).toUpperCase() + n.toString().slice(1));
