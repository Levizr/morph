// 30_late_values - late-assigned bindings keep JS semantics
//
// `let x;` and `let x = null/undefined` hold `undefined` until the first
// assignment, then accept any value (number, string, boolean, object,
// array) with real JS operator behavior. Nothing here needs a TypeScript
// annotation or a C++ escape hatch.

// Read before assignment is `undefined`; later numeric use works.
let count;
console.log(count);
count = 5;
console.log(count + 1);
console.log(count - 2);
console.log(count * 3);
console.log(count / 2);

// Compound assignment and increment/decrement.
let total;
total = 10;
total += 5;
console.log(total);
total *= 2;
console.log(total);
total++;
console.log(total);
total--;
console.log(total);

// Strings and string methods after a late assignment.
let label;
label = "hello";
console.log(label + "!");
console.log(label.toUpperCase());
console.log(label.length);

// Objects and arrays.
let user;
user = { name: "Ada", age: 36 };
console.log(user.name);
console.log(user.age + 1);

let list;
list = [1, 2, 3];
console.log(list.length);
console.log(list[0] + list[1]);
console.log(list.includes(2));

// Nullish declarations behave the same.
let score = null;
console.log(score);
score = 7;
console.log(score + 3);

let ratio = undefined;
ratio = 2.5;
console.log(ratio * 2);

// Unary and bitwise operators use ToNumber.
let bits;
bits = 5;
console.log(-bits);
console.log(~bits);
console.log(bits & 3);
console.log(bits | 2);
console.log(bits ^ 1);

// A boolean late binding.
let flag;
console.log(flag);
flag = true;
console.log(flag);
console.log(!flag);

// Reads before assignment follow JS coercion.
let missing;
console.log(missing + 1);

// Logical assignment and increments coerce like JS.
let maybe = null;
maybe ??= "fallback";
console.log(maybe);

let word;
word = "5";
word++;
console.log(word);

let zero;
zero = 0;
zero ||= 4;
console.log(zero);
