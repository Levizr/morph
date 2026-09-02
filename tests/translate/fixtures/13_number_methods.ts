// 13_number_methods - test JsNumber operations
let a: number = 10;
let b: number = 3;
console.log(a + b);
console.log(a - b);
console.log(a * b);
console.log(a / b);
console.log(a % b);

let x: number = 5;
x += 10;
console.log(x);
x -= 3;
console.log(x);
x *= 2;
console.log(x);
x /= 2;
console.log(x);

let n: number = 42;
console.log(n.toString());
console.log(`${n}`);
console.log(`Number is ${n}`);

let big: number = 100;
let small: number = 2;
console.log(big > small);
console.log(big < small);
console.log(big === 100);
console.log(big !== 99);

let neg: number = -5;
console.log(-neg);
console.log(+neg);

let inc: number = 0;
inc++;
console.log(inc);
++inc;
console.log(inc);
inc--;
console.log(inc);
--inc;
console.log(inc);

let bitwiseA: number = 5;
let bitwiseB: number = 3;
console.log(bitwiseA & bitwiseB);
console.log(bitwiseA | bitwiseB);
console.log(bitwiseA ^ bitwiseB);

function add(x: number, y: number): number {
    return x + y;
}
console.log(add(10, 20));
console.log(add(add(1, 2), 3));

let arr: number[] = [10, 20, 30];
console.log(arr.length);
console.log(arr[0] + arr[1]);

let obj = {value: 100};
console.log(obj["value"]);
console.log(obj["value"] + 50);
