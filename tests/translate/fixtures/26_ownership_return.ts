// 26_ownership_return - returned class instances move out as unique_ptr;
// a factory result with shared use wraps in shared_ptr instead.
class User {
    name: string = "";
}

function createUser(nm: string): User {
    const u = new User();
    u.name = nm;
    return u;
}

async function main(): Promise<void> {
    const admin = createUser("Ada");
    console.log(admin.name);
    const guest = createUser("Bo");
    console.log(guest.name);
    console.log(guest.name);
}
main();
