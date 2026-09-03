// 04_arrays - literals, spread, nested, push, length, iteration
let arr: number[] = [1, 2, 3];
console.log(arr.length);
console.log(arr[0]);
console.log(arr[1]);

let arr2: number[] = [1, 2, 3];
arr2.push(4);
console.log(arr2.length);
console.log(arr2[3]);

let nested: number[][] = [[1, 2], [3, 4]];
console.log(nested.length);
console.log(nested[0].length);

let spread: number[] = [1, 2, 3, 4];
console.log(spread.length);
console.log(spread[2]);

let empty: number[] = [];
console.log(empty.length);
empty.push(10);
console.log(empty[0]);

// sum via index
let sum: number = 0;
for (let i: number = 0; i < arr.length; i++) {
    sum = sum + arr[i];
}
console.log(sum);

// for loop with array
let sum2: number = 0;
for (let i: number = 0; i < arr.length; i++) {
    sum2 = sum2 + arr[i];
}
console.log(sum2);

let strArr: string[] = ["a", "b", "c"];
console.log(strArr.length);
console.log(strArr[1]);

let mixed: any[] = [1, "hello", true];
console.log(mixed.length);
