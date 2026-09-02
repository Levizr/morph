// 06_control_flow - if/else, ternary, switch, loops, break/continue
let x: number = 10;
if (x > 5) {
    console.log("big");
} else {
    console.log("small");
}

let y: number = 3;
if (y > 10) {
    console.log("a");
} else if (y > 2) {
    console.log("b");
} else {
    console.log("c");
}

let a: number = 5;
let b: number = 10;
let max: number = a > b ? a : b;
console.log(max);

let level: number = 2;
switch (level) {
    case 0:
        console.log("zero");
        break;
    case 1:
        console.log("one");
        break;
    case 2:
        console.log("two");
        break;
    default:
        console.log("other");
        break;
}

let sum: number = 0;
for (let i: number = 0; i < 5; i++) {
    sum += i;
}
console.log(sum);

let j: number = 0;
while (j < 3) {
    console.log(j);
    j++;
}

let k: number = 0;
do {
    console.log(k);
    k++;
} while (k < 2);

for (let i: number = 0; i < 10; i++) {
    if (i === 5) break;
    console.log(i);
}

for (let i: number = 0; i < 5; i++) {
    if (i % 2 === 0) continue;
    console.log(i);
}

let n: number = 0;
for (; n < 3; n++) {
    console.log(n);
}

// sum via index (avoid for..of)
let arr: number[] = [1, 2, 3];
let total: any = 0;
for (let i: number = 0; i < arr.length; i++) {
    let val: any = arr[i];
    total = total + val;
}
console.log(total);

let obj = {a: 1, b: 2, c: 3};
console.log(obj["a"]);
console.log(obj["b"]);
