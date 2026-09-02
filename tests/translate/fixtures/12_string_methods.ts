// 12_string_methods - test JsString methods
let s: string = "hello world";
console.log(s.length);
console.log(s.toUpperCase());
console.log(s.toLowerCase());
console.log(s.charAt(0));
console.log(s.charAt(1));
console.log(s.indexOf("world"));
console.log(s.indexOf("hello"));
console.log(s.substring(0, 5));
console.log(s.slice(6, 11));
console.log(s.trim());
console.log(s.replace("world", "there"));
console.log(s.split(" ").length);
let upper: string = "ABC";
console.log(upper.toLowerCase());
let lower: string = "xyz";
console.log(lower.toUpperCase());

let spaced: string = "  trim me  ";
console.log(spaced.trim());

let withSpaces: string = "a b c";
let parts = withSpaces.split(" ");
console.log(parts[0]);
console.log(withSpaces.split(" ").length);
let literalStr: string = "a b c";
console.log(literalStr.split(" ").length);

let numStr: string = "123";
let num: number = 42;
let combined: string = `${numStr} and ${num}`;
console.log(combined);

let a: string = "Hello";
let b: string = "World";
let c: string = `${a}, ${b}!`;
console.log(c);
console.log(c.toUpperCase());
console.log(c.length);
