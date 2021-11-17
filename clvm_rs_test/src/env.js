let malloc_at = 0x1000;

module.exports = {
    clvm_rs: undefined,
    malloc: function(n) {
        let next = malloc_at;
        malloc_at += (n + 16) & ~15;
        return next;
    },
    realloc: function(a,m) {
        console.log(a,m,this.clvm_rs);
        throw 'realloc';
    },
    fwrite: function() {
    },
    free: function(x) { },
    fiprintf: function() { },
    abort: function() { }
};
