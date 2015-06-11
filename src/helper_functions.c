#include "helper_functions.h"


/*
 * Helper functions for tree
 */

xmlNodePtr xmlNextSibling(const xmlNodePtr cur) {
    return cur->next;
}

xmlNodePtr xmlPrevSibling(const xmlNodePtr cur) {
    return cur->prev;
}

xmlNodePtr xmlGetFirstChild(const xmlNodePtr cur) {
    return cur->children;
}

int xmlIsTextNode(const xmlNodePtr cur) {
    return cur->type == XML_TEXT_NODE ? 1 : 0;
}

const char * xmlNodeGetName(const xmlNodePtr cur) {
    return (char *) cur->name;
}

const char * xmlNodeGetContentPointer(const xmlNodePtr cur) {
    return (char *) cur->content;
}



/*
 * helper functions for xpath
 */

int xmlXPathObjectNumberOfNodes(const xmlXPathObjectPtr val) {
    return val->nodesetval->nodeNr;
}

xmlNodePtr xmlXPathObjectGetNode(const xmlXPathObjectPtr val, size_t index) {
    return val->nodesetval->nodeTab[index];
}

void xmlFreeXPathObject(xmlXPathObject val) {
    xmlFree(val->nodesetval->nodeTab);
    xmlFree(val->nodesetval);
    xmlFree(val);
}
